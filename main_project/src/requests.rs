use std::collections::HashMap;
use serde_json::to_string_pretty;
use crate::{config::NUM_FLOORS, types::{ElevatorFSM, Event, Order}};
use tokio::process::Command;
use std::process::Stdio;
use crate::types::*;

impl RequestAssigner {
    pub async fn new(id: String, role: Roles, message: Message) -> Self {
        Self { message, id, role }
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
        
        // Check if a master already exists (including self)
        if matches!(self.role, Roles::Master) {
            println!("I am already the master: {}", self.id);
            // Verify that no other node claims to be master
            if let Some(other_master) = gossip_heartbeats
                .iter()
                .find(|hb| matches!(hb.role, Roles::Master)) {
                println!("WARNING: Another master detected: {}. Demoting to slave.", other_master.id);
                self.role = Roles::Slave;
                network.msg.role = Roles::Slave;
                network.msg.counter += 1;
            }
            return;
        }
        
        if let Some(master) = gossip_heartbeats
            .iter()
            .find(|hb| matches!(hb.role, Roles::Master)) {
            println!("Master already exists: {}", master.id);
            self.role = Roles::Slave;
            network.msg.role = Roles::Slave;
            network.msg.counter += 1;
            return;
        }
        
        // Build a list including own ID and all gossip IDs
        let mut all_ids: Vec<String> = vec![self.id.clone()];
        for hb in &gossip_heartbeats {
            all_ids.push(hb.id.clone());
        }
        
        if let Some(min_id) = all_ids.iter().min_by_key(|id| {
            IpAddr::from_str(id).unwrap_or(IpAddr::from([0, 0, 0, 0]))
        }) {
            if *min_id == self.id {
                println!("I am elected as master: {}", self.id);
                self.role = Roles::Master;
                network.msg.role = Roles::Master;
                network.msg.counter += 1;
            } else {
                println!("Master elected: {} (I am {})", min_id, self.id);
                self.role = Roles::Slave;
                network.msg.role = Roles::Slave;
                network.msg.counter += 1;
            }
        }
    }

    pub async fn master(&self) {
        //TODO call the cost function, send gossip, and its own orders to its fsm
    }

    pub async fn slave(&self) {
        //TODO recieve orders and send to fsm, check gossip 
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
