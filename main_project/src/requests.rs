use std::collections::HashMap;
use serde_json::to_string_pretty;
use crate::{config::NUM_FLOORS, types::{ElevatorFSM, Event, Order}};
use tokio::process::Command;
use std::process::Stdio;
use crate::types::*;

impl RequestAssigner {
    pub async fn new(id: String, role: Roles, message: Message) -> Self {
        Self { message, id, role, last_published_assignments: HashMap::new(), }
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
            cab_requests: msg
                .internal_orders()
                .iter()
                .map(|o| matches!(o.order_type, ButtonType::CabCall))
                .collect(),
        };

        self.message.states.insert(msg.id().to_string(), new_state);
    }

    pub async fn cost_function(&self) -> HashMap<String, Vec<Order>> {
    let json_str = serde_json::to_string_pretty(&self.message).unwrap();
    println!("Message: {}", json_str);

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
        self.message.hall_requests = vec![[false, false]; num_floors];

        // 1) legg inn master sin egen state først (ALLTID)
        self.insert_state_from_hb(own_hb, num_floors);
        self.union_hall_from_hb(own_hb, num_floors);

        // 2) legg inn alle andre fra gossip
        for hb in gossip {
            self.insert_state_from_hb(hb, num_floors);
            self.union_hall_from_hb(hb, num_floors);
        }
    }

    fn insert_state_from_hb(&mut self, hb: &HeartbeatMSG, num_floors: usize) {
        let mut cab = vec![false; num_floors];
        for o in hb.internal_orders.iter() {
            if matches!(o.order_type, ButtonType::CabCall) {
                let f = o.floor as usize;
                if f < num_floors { cab[f] = true; }
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
            cab_requests: cab,
        };

        self.message.states.insert(hb.id.clone(), st);
    }

    fn union_hall_from_hb(&mut self, hb: &HeartbeatMSG, num_floors: usize) {
        for o in hb.external_orders.iter() {
            let f = o.floor as usize;
            if f >= num_floors { continue; }
            match o.order_type {
                ButtonType::HallUp => self.message.hall_requests[f][0] = true,
                ButtonType::HallDown => self.message.hall_requests[f][1] = true,
                _ => {}
            }
        }
    }

    pub async fn elect_master(&mut self, gossip_heartbeats: Vec<HeartbeatMSG>, network: &mut Heartbeat) {
        use std::net::IpAddr;
        use std::str::FromStr;

        let mut all_ids: Vec<String> = vec![self.id.clone()];
        for hb in &gossip_heartbeats {
            all_ids.push(hb.id.clone());
        }

        let elected = all_ids
            .iter()
            .min_by_key(|id| IpAddr::from_str(id).unwrap_or(IpAddr::from([255, 255, 255, 255])))
            .unwrap()
            .clone();

        let new_role = if elected == self.id { Roles::Master } else { Roles::Slave };

        let role_changed =
            std::mem::discriminant(&self.role) != std::mem::discriminant(&new_role);

        if role_changed {
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
        fsm: &mut ElevatorFSM,
    ) {
        self.build_message_from_gossip(gossip, &network.msg);

        let assignments = self.cost_function().await;

        // Publiser bare hvis assignments faktisk endrer seg
        if assignments != self.last_published_assignments {
            self.last_published_assignments = assignments.clone();
            network.msg.assignments = assignments.clone();
            network.msg.counter += 1;

            println!("\n--- ASSIGNMENTS (published) ---");
            for (id, orders) in &assignments {
                print!("{}: ", id);
                for o in orders {
                    print!("[f{} {:?}] ", o.floor, o.order_type);
                }
                println!();
            }
        }

        // MASTER: ta egne ordre direkte fra network.msg.assignments
        self.apply_my_assignments_from_map(&network.msg.assignments, network.msg.counter, network, fsm, true);
    }

    /// SLAVE: finn master i gossip og bruk assignments derfra
    pub async fn slave(&mut self, gossip: &[HeartbeatMSG], network: &Heartbeat, fsm: &mut ElevatorFSM) {
        if let Some(master_hb) = gossip.iter().find(|hb| matches!(hb.role, Roles::Master)) {
            self.apply_my_assignments_from_map(&master_hb.assignments, master_hb.counter, network, fsm, false);
        }
    }

    /// Felles: plukk ut mine orders fra et assignment-map og legg dem i køen
    fn apply_my_assignments_from_map(
        &self,
        assignments_map: &HashMap<String, Vec<Order>>,
        assignments_counter: i32,
        network: &Heartbeat,
        fsm: &mut ElevatorFSM,
        is_master: bool,
    ) {
        let my_id = network.id().to_string();

        let Some(my_orders) = assignments_map.get(&my_id) else { return; };

        // Hvis vi allerede har behandlet denne batchen, gjør ingenting
        if assignments_counter == fsm.last_received_msg_counter {
            return;
        }

        fsm.last_received_msg_counter = assignments_counter;

        // Dedupe: legg kun inn orders som ikke allerede ligger i queue
        for o in my_orders {
            if !fsm.queue.contains(o) {
                if is_master {
                    println!("(MASTER) enqueue order: f{} {:?}", o.floor, o.order_type);
                } else {
                    println!("(SLAVE)  enqueue order: f{} {:?}", o.floor, o.order_type);
                }
                fsm.queue.push(o.clone());
            }
        }
    }

    pub async fn send_to_own_fsm(&self, fsm: &mut ElevatorFSM, heartbeat: HeartbeatMSG) {
        if heartbeat.counter == fsm.last_received_msg_counter {
            println!("Duplicate message with counter {}, skipping", heartbeat.counter);
            return;
        }
        
        fsm.last_received_msg_counter = heartbeat.counter;
        println!("send_to_own_fsm called with {} orders (counter: {})", heartbeat.external_orders.len(), heartbeat.counter);
        for order in heartbeat.external_orders {
            println!("Adding order to queue: floor {}", order.floor);
            fsm.queue.push(order);
        }
    }

}
