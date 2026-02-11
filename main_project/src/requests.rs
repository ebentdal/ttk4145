use std::collections::HashMap;
use tokio::process::Command;
use std::process::Stdio;
use crate::types::{
    Behaviour, Direction, ElevatorState, Heartbeat, Message, RequestAssigner, Roles,
};

impl RequestAssigner {
    pub async fn new(id: String, role: Roles, message: Message) -> Self {
        Self { message, id, role }
    }

    pub async fn process_heartbeat(&mut self, msg: Heartbeat) {
        let new_state = ElevatorState {
            behaviour: msg.status().clone(),
            floor: msg.floor(),
            direction: match msg.direction() {
                0 => Direction::Stop,
                1 => Direction::Up,
                2 => Direction::Down,
                _ => Direction::Stop,
            },
            cab_requests: msg.internal_orders().iter().map(|&x| x == 1).collect(),
        };

        self.message.states.insert(msg.id().to_string(), new_state);
    }

    pub async fn order_assigner(&self) {
        let id1 = ElevatorState {
            behaviour: Behaviour::Moving,
            floor: 2,
            direction: Direction::Up,
            cab_requests: vec![false, false, true, true],
        };

        let id2 = ElevatorState {
            behaviour: Behaviour::Idle,
            floor: 0,
            direction: Direction::Stop,
            cab_requests: vec![false, false, false, false],
        };

        let mut states = HashMap::new();
        states.insert("one".to_string(), id1);
        states.insert("two".to_string(), id2);

        let msg = Message {
            hall_requests: vec![[false, false], [true, false], [false, false], [false, true]],
            states,
        };

        let json_str = serde_json::to_string_pretty(&msg).unwrap();
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
        let formattedoutput = String::from_utf8_lossy(&output.stdout);
        let prettyoutput: String = serde_json::to_string_pretty(&formattedoutput).unwrap();

        println!("Stdout: {}", prettyoutput);
    }
}
