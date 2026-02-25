use std::collections::HashMap;
use serde_json::to_string_pretty;
use crate::{config::NUM_FLOORS, types::{ElevatorFSM, Event, Order}};
use tokio::process::Command;
use std::process::Stdio;
use crate::types::*;
use std::net::IpAddr;
use std::str::FromStr;
use tokio::time::Instant;

use std::sync::Arc;

impl RequestAssigner {
    pub async fn new(id: String, role: Roles, message: Message) -> Self {
        Self { message, id, role, last_published_assignments: HashMap::new(), last_seen: HashMap::new(), peer_ttl: tokio::time::Duration::from_secs(2),} //2 second timeout for master election
    }

    pub async fn process_heartbeat(&mut self, msg: Heartbeat) {
        let id = msg.id();
        let new_state = ElevatorState {
            behaviour: msg.status().clone(),
            floor: msg.floor(),
            direction: match msg.direction() {
                0 => Direction::Stop,
                1 => Direction::Up,
                2 => Direction::Down,
                _ => Direction::Stop,
            },
            cabRequests: msg
                .internal_orders()
                .iter()
                .map(|o| matches!(o.order_type, ButtonType::CabCall))
                .collect(),
        };

        self.message.states.insert(msg.id().to_string(), new_state);
    }

    pub async fn cost_function(&self) -> HashMap<String, Vec<Order>> {
    let json_str = serde_json::to_string_pretty(&self.message).unwrap();
    println!("[COST] input json:\n{}", json_str);

    let child = Command::new("./hall_request_assigner")
        .arg("--input")
        .arg(&json_str)
        .arg("--includeCab") // behold som du har (da får vi 3 kolonner)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let output = child.wait_with_output().await.unwrap();

    if !output.status.success() {
        eprintln!(
            "hall_request_assigner feilet. stderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return HashMap::new();
    }

    let raw: HashMap<String, Vec<Vec<bool>>> = serde_json::from_slice(&output.stdout).unwrap();
    println!("[COST] raw cost output: {:?}\n", raw);

    let mut assignments: HashMap<String, Vec<Order>> = HashMap::new();

    for (id, per_floor) in raw {
        let mut orders: Vec<Order> = Vec::new();

        for (floor, cols) in per_floor.iter().enumerate() {
            if cols.get(0).copied().unwrap_or(false) {
                orders.push(Order {
                    floor: floor as u8,
                    order_type: ButtonType::HallUp,
                });
            }
            if cols.get(1).copied().unwrap_or(false) {
                orders.push(Order {
                    floor: floor as u8,
                    order_type: ButtonType::HallDown,
                });
            }
            if cols.get(2).copied().unwrap_or(false) {
                orders.push(Order {
                    floor: floor as u8,
                    order_type: ButtonType::CabCall,
                });
            }
        }
        assignments.insert(id, orders);
    }
    assignments
    }

    pub fn build_message_from_gossip(
        &mut self,
        gossip: &[HeartbeatMSG],
        own_hb: &HeartbeatMSG,
    ) {
        let num_floors = crate::config::NUM_FLOORS as usize;

        self.message.states.clear();
        self.message.hallRequests = vec![[false, false]; num_floors];

        self.insert_state_from_hb(own_hb, num_floors);
        self.union_hall_from_hb(own_hb, num_floors);

        for hb in gossip {
            self.insert_state_from_hb(hb, num_floors);
            self.union_hall_from_hb(hb, num_floors);
        }
    }

    fn insert_state_from_hb(&mut self, hb: &HeartbeatMSG, num_floors: usize) {
        let mut cab = vec![false; num_floors];
        for order in hb.internal_orders.iter() {
            if matches!(order.order_type, ButtonType::CabCall) {
                let floor = order.floor as usize;
                if floor < num_floors { cab[floor] = true; }
            }
        }

        let st = ElevatorState {
            behaviour: hb.status.clone(),
            floor: hb.floor,
            direction: match hb.direction {
                0 => Direction::Stop,
                1 => Direction::Up,
                2 => Direction::Down,
                _ => Direction::Stop,
            },
            cabRequests: cab,
        };

        self.message.states.insert(hb.id.clone(), st);
    }

    fn union_hall_from_hb(&mut self, hb: &HeartbeatMSG, num_floors: usize) {
        for order in hb.external_orders.iter() {
            let floor = order.floor as usize;
            if floor >= num_floors { continue; }
            match order.order_type {
                ButtonType::HallUp => self.message.hallRequests[floor][0] = true,
                ButtonType::HallDown => self.message.hallRequests[floor][1] = true,
                _ => {}
            }
        }
    }

    pub async fn elect_master(
        &mut self,
        gossip_heartbeats: Vec<HeartbeatMSG>,
        network: &mut Heartbeat,
    ) 
    {
        let now = Instant::now();

        self.last_seen.insert(self.id.clone(), now);
        for hb in &gossip_heartbeats {
            self.last_seen.insert(hb.id.clone(), now);
        }

        let ttl = self.peer_ttl;
        self.last_seen.retain(|_, t| now.duration_since(*t) <= ttl);

        let mut candidates: Vec<String> = self.last_seen.keys().cloned().collect();
        if !candidates.iter().any(|id| id == &self.id) {
            candidates.push(self.id.clone());
        }

        let elected = candidates
            .iter()
            .min_by_key(|id| IpAddr::from_str(id).unwrap_or(IpAddr::from([255, 255, 255, 255])))
            .unwrap()
            .clone();

        let new_role = if elected == self.id {
            Roles::Master
        } else {
            Roles::Slave
        };

        if self.role != new_role {
            println!(
                "Role change: {:?} -> {:?} (elected master: {})",
                self.role, new_role, elected
            );
            self.role = new_role.clone();
            network.msg.role = new_role;
            network.msg.counter += 1;
        } else {
            network.msg.role = new_role;
        }
    }

    pub async fn master(
        &mut self,
        gossip: &[HeartbeatMSG],
        network: &mut Heartbeat,
        fsm: Arc<ElevatorFSM>,
    ) {
        self.build_message_from_gossip(gossip, &network.msg);

        let assignments = self.cost_function().await;
        println!("[MASTER] computed assignments: {:?}", assignments);

        // if the cost function returned no orders for us, fall back to serving
        // any external orders in our own heartbeat so we don't deadlock when the
        // assigner misbehaves or the IDs mismatch across machines.
        let self_id = network.id().to_string();
        if !assignments.contains_key(&self_id) && !network.msg.external_orders.is_empty() {
            println!("[MASTER] fallback: enqueuing {} local external orders", network.msg.external_orders.len());
            let mut q = fsm.queue.lock().await;
            for order in &network.msg.external_orders {
                if !q.contains(order) {
                    q.push(order.clone());
                }
            }
        }

        // Publiser bare hvis assignments faktisk endrer seg
        if assignments != self.last_published_assignments {
            self.last_published_assignments = assignments.clone();
            network.msg.assignments = assignments.clone();
            network.msg.counter += 1;

            println!("\n--- ASSIGNMENTS (published) ---");
            for (id, orders) in &assignments {
                print!("{}: ", id);
                for order in orders {
                    print!("[f{} {:?}] ", order.floor, order.order_type);
                }
                println!();
            }
        }

        self.apply_my_assignments_from_map(&network.msg.assignments, network.msg.counter, network, &fsm, true).await;
    }

    pub async fn slave(&mut self, gossip: &[HeartbeatMSG], network: &Heartbeat, fsm: Arc<ElevatorFSM>) {
        if let Some(master_hb) = gossip.iter().find(|hb| matches!(hb.role, Roles::Master)) {
            println!("[SLAVE] received master heartbeat with assignments {:?} (counter {})", master_hb.assignments, master_hb.counter);
            self.apply_my_assignments_from_map(&master_hb.assignments, master_hb.counter, network, &fsm, false).await;
        } else {
            println!("[SLAVE] no master heartbeat found in gossip");
        }
    }

    async fn apply_my_assignments_from_map(
        &self,
        assignments_map: &HashMap<String, Vec<Order>>,
        assignments_counter: i32,
        network: &Heartbeat,
        fsm: &Arc<ElevatorFSM>,
        is_master: bool,
    ) {
        let my_id = network.id().to_string();

        let Some(my_orders) = assignments_map.get(&my_id) else { return; };

        // update counter under inner lock
        {
            let mut inner = fsm.inner.lock().await;
            if assignments_counter == inner.last_received_msg_counter {
                return;
            }
            inner.last_received_msg_counter = assignments_counter;
        }

        // enqueue orders under queue lock
        let mut q = fsm.queue.lock().await;
        for order in my_orders {
            if !q.contains(order) {
                if is_master {
                    println!("(MASTER) enqueue order: f{} {:?}", order.floor, order.order_type);
                } else {
                    println!("(SLAVE)  enqueue order: f{} {:?}", order.floor, order.order_type);
                }
                q.push(order.clone());
            }
        }
    }

    pub async fn send_to_own_fsm(&self, fsm: &Arc<ElevatorFSM>, heartbeat: HeartbeatMSG) {
        // update counter
        {
            let mut inner = fsm.inner.lock().await;
            if heartbeat.counter == inner.last_received_msg_counter {
                println!("Duplicate message with counter {}, skipping", heartbeat.counter);
                return;
            }
            inner.last_received_msg_counter = heartbeat.counter;
        }
        println!("send_to_own_fsm called with {} orders (counter: {})", heartbeat.external_orders.len(), heartbeat.counter);
        let mut q = fsm.queue.lock().await;
        for order in heartbeat.external_orders {
            println!("Adding order to queue: floor {}", order.floor);
            q.push(order);
        }
    }

}