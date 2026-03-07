use std::collections::HashMap;
use crate::types::{ElevatorFSM, Order};
use tokio::process::Command;
use std::process::Stdio;
use crate::types::*;
use std::net::IpAddr;
use std::str::FromStr;
use tokio::time::Instant;

use std::sync::Arc;

impl RequestAssigner {

    pub fn new(id: String, role: Roles, message: Message) -> Self {
        Self {
            message,
            id,
            role,
            last_published_assignments: HashMap::new(),
            last_seen: HashMap::new(),
            peer_ttl: tokio::time::Duration::from_secs(2),
        }
    }

    pub async fn cost_function(&self) -> HashMap<String, Vec<Order>> {
        let json_str = serde_json::to_string_pretty(&self.message).unwrap();
        println!("{:#?}", json_str);
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
            println!(
                "hall_request_assigner feilet. stderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return HashMap::new();
        }

        let raw: HashMap<String, Vec<Vec<bool>>> = serde_json::from_slice(&output.stdout).unwrap();

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
        own_heartbeat: &HeartbeatMSG,
    ) {
        let num_floors = crate::config::NUM_FLOORS as usize;

        self.message.states.clear();
        self.message.hall_requests = vec![[false, false]; num_floors];

        for heartbeat in std::iter::once(own_heartbeat).chain(gossip) {
            self.insert_state_from_heartbeat(heartbeat, num_floors);
            self.merge_external_orders(heartbeat, num_floors);
        }
    }

    fn insert_state_from_heartbeat(&mut self, heartbeat: &HeartbeatMSG, num_floors: usize) {
        let mut cab = vec![false; num_floors];
        for order in heartbeat.internal_orders.iter() {
            if matches!(order.order_type, ButtonType::CabCall) {
                let floor = order.floor as usize;
                if floor < num_floors { cab[floor] = true; }
            }
        }

        let st = ElevatorState {
            behaviour: heartbeat.status.clone(),
            floor: heartbeat.floor,
            direction: match heartbeat.direction {
                0 => Direction::Stop,
                1 => Direction::Up,
                2 => Direction::Down,
                _ => Direction::Stop,
            },
            cab_requests: cab,
        };

        self.message.states.insert(heartbeat.id.clone(), st);
    }

    fn merge_external_orders(&mut self, heartbeat: &HeartbeatMSG, num_floors: usize) {
        for order in heartbeat.external_orders.iter() {
            let floor = order.floor as usize;
            if floor >= num_floors { continue; }
            match order.order_type {
                ButtonType::HallUp => self.message.hall_requests[floor][0] = true,
                ButtonType::HallDown => self.message.hall_requests[floor][1] = true,
                _ => {}
            }
        }
    }

    async fn enqueue_orders(
        &self,
        fsm: &Arc<ElevatorFSM>,
        orders: &[Order],
        counter: i32,
    ) {
        {
            let mut inner = fsm.inner.lock().await;
            if counter == inner.last_received_msg_counter {
                return;
            }
            inner.last_received_msg_counter = counter;
        }

        let currently_serving = {
            let inner = fsm.inner.lock().await;
            inner.currently_serving.clone()
        };

        // Replace the queue with the new assignment from the cost function,
        // skipping only the order currently being executed.
        let mut q = fsm.queue.lock().await;
        q.clear();
        for order in orders {
            if currently_serving.as_ref() != Some(order) {
                q.push(order.clone());
            }
        }
    }   

    pub async fn elect_master(
        &mut self, 
        gossip_heartbeats: Vec<HeartbeatMSG>, 
        network: &mut Heartbeat,
    ) {
        
        let now = Instant::now();

        self.last_seen.insert(self.id.clone(), now);
        for heartbeat in &gossip_heartbeats {
            self.last_seen.insert(heartbeat.id.clone(), now);
        }

        let ttl = self.peer_ttl;
        self.last_seen.retain(|_, t| now.duration_since(*t) <= ttl);

        let candidates: Vec<String> = self.last_seen.keys().cloned().collect();

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
        // Remove orders marked as completed in heartbeats
        for heartbeat in gossip {
            if let Some(cleared) = &heartbeat.cleared_order {
                network.msg.external_orders.retain(|order| order != cleared);
                network.msg.internal_orders.retain(|order| order != cleared);
            }
        }
        
        // Also check own completed orders
        if let Some(cleared) = &network.msg.cleared_order {
            network.msg.external_orders.retain(|order| order != cleared);
            network.msg.internal_orders.retain(|order| order != cleared);
        }

        self.build_message_from_gossip(gossip, &network.msg);

        let assignments = self.cost_function().await;
        println!("{:#?}",assignments);

        let self_id = network.id().to_string();
        if !assignments.contains_key(&self_id) && !network.msg.external_orders.is_empty() {
            let mut q = fsm.queue.lock().await;
            for order in &network.msg.external_orders {
                if !q.contains(order) {
                    q.push(order.clone());
                }
            }
        }

        if assignments != self.last_published_assignments {
            self.last_published_assignments = assignments.clone();
            network.msg.assignments = assignments.clone();
            network.msg.counter += 1;
        }

        if let Some(my_orders) = network.msg.assignments.get(&self_id) {
            self.enqueue_orders(&fsm, my_orders, network.msg.counter).await;
        }
        
        // Update button lights to match all orders (external + internal)
        fsm.set_button_light(&network.msg.external_orders, &network.msg.internal_orders).await;
    }

    pub async fn slave(&mut self, gossip: &[HeartbeatMSG], network: &Heartbeat, fsm: Arc<ElevatorFSM>) {
        if let Some(master_heartbeat) = gossip.iter().find(|heartbeat| matches!(heartbeat.role, Roles::Master)) {
            let my_id = network.id().to_string();
            if let Some(my_orders) = master_heartbeat.assignments.get(&my_id) {
                self.enqueue_orders(&fsm, my_orders, master_heartbeat.counter).await;
            }

            // Hall lights from master (shared), cab lights from own orders only
            fsm.set_button_light(&master_heartbeat.external_orders, &network.msg.internal_orders).await;
        }
    }
}