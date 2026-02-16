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
            .arg("--includeCab")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let output = child.wait_with_output().await.unwrap();
        
        let assignments: HashMap<String, Vec<Order>> = serde_json::from_slice(&output.stdout).unwrap();
        //let printAssignments = to_string_pretty(&assignments);
        // let formattedoutput = String::from_utf8_lossy(&output.stdout);
        // let prettyoutput: String = serde_json::to_string_pretty(&formattedoutput).unwrap();
        //println!("Stdout: {}", printAssignments);
        return assignments;
    }

    pub async fn master(&self) {
        //TODO call the cost function, send gossip, and its own orders to its fsm
    }

    pub async fn slave(&self) {
        //TODO recieve orders and send to fsm, check gossip 
    }

    pub async fn send_to_own_fsm(&self, fsm: &mut ElevatorFSM, heartbeat: HeartbeatMSG) {
        // Skip if this is a duplicate message (same counter as before)
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

        // let id1 = ElevatorState {
        //     behaviour: Behaviour::Moving,
        //     floor: 2,
        //     direction: Direction::Up,
        //     cab_requests: vec![false, false, true, true],
        // };

        // let id2 = ElevatorState {
        //     behaviour: Behaviour::Idle,
        //     floor: 0,
        //     direction: Direction::Stop,
        //     cab_requests: vec![false, false, false, false],
        // };