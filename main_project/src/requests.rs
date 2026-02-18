use std::collections::HashMap;
use serde_json::to_string_pretty;
use crate::types::{ElevatorFSM, Event, Order};
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
            cab_requests: {
                let mut v = vec![false; crate::config::NUM_FLOORS as usize];
                for o in msg.internal_orders().iter() {
                    if matches!(o.order_type, ButtonType::CabCall) {
                        if (o.floor as usize) < v.len() {
                            v[o.floor as usize] = true;
                        }
                    }
                }
                v
            },
        };

        self.message.states.insert(msg.id().to_string(), new_state);
    }

    pub async fn cost_function(&self) -> HashMap<String, Vec<Order>> {
        let json_str = serde_json::to_string_pretty(&self.message).unwrap();
        println!("Message: {}", json_str);

        let child = match Command::new("./hall_request_assigner")
            .arg("--input")
            .arg(&json_str)
            .arg("--includeCab")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("failed to spawn hall_request_assigner: {}", e);
                return HashMap::new();
            }
        };

        let output = match child.wait_with_output().await {
            Ok(o) => o,
            Err(e) => {
                eprintln!("failed to wait for hall_request_assigner: {}", e);
                return HashMap::new();
            }
        };

        if !output.status.success() {
            eprintln!(
                "hall_request_assigner failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return HashMap::new();
        }

        if output.stdout.is_empty() {
            eprintln!("hall_request_assigner produced no stdout");
            return HashMap::new();
        }

        let assignments: HashMap<String, Vec<Order>> = match serde_json::from_slice(&output.stdout) {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "failed to parse hall_request_assigner output: {}\nstdout: {}\nstderr: {}",
                    e,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                );
                return HashMap::new();
            }
        };

        assignments
    }
    pub async fn elect_master(&mut self, gossip_heartbeats: Vec<HeartbeatMSG>, network: &mut Heartbeat) {
        use std::net::IpAddr;
        use std::str::FromStr;
        
        if matches!(self.role, Roles::Master) {
            println!("I am already the master: {}", self.id);
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

