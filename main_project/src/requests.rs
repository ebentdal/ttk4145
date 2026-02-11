use serde_json::Result;
use std::collections::HashMap;
use tokio::process::Command;
use std::process::Stdio;
use serde::Serialize;

use crate::networkhandler::Heartbeat;
use crate::networkhandler::Status;

#[derive(Serialize)]
pub struct Message {
    hallRequests: Vec<[bool; 2]>,
    states: HashMap<String, TestState>,
}

#[derive(Serialize)]
pub struct TestState {
    behaviour: behaviour,
    floor: u8,
    direction: Direction,
    cabRequests: Vec<bool>,
}

#[derive(Serialize)]
pub enum behaviour {
    idle,
    moving,
    doorOpen,
}

#[derive(Serialize)]
pub enum Direction {
    up,
    down,
    stop,
}

pub struct RequestAssigner{
    message: Message,
    id: String,
    role: Roles,
}

pub enum Roles {
    Master,
    Slave,
}

impl RequestAssigner {
    
    pub async fn new(id: String, role: Roles, message: Message) -> Self {
        Self { message, id, role}
    }

    pub async fn process_hearbeat(&mut self, msg: Heartbeat){
//         pub struct HeartbeatMSG{
//     ID: String,
//     ExternalOrders: Vec<u8>,
//     InternalOrders: Vec<u8>,
//     Floor: u8,
//     Direction: u8,
//     StatusFlag: Status,
//     counter: i32,
//     Role: Roles,
// }
        let new_state = TestState {
        behaviour: match msg.status() {
            Status::Idle => behaviour::idle,
            Status::Moving => behaviour::moving,
            Status::DoorOpen => behaviour::doorOpen,
        },
        floor: msg.floor(),
        direction: match msg.direction(){
            0 => Direction::stop,
            1 => Direction::up,
            2 => Direction::down,
            _ => Direction::stop,
        },
        cabRequests: msg.internalOrders()
            .iter()
            .map(|&x| x == 1)
            .collect(),
    };

    self.message.states.insert(msg.id().clone().to_string(), new_state);

    }

    pub async fn orderAssigner (&self) {
        let id1 = TestState {
            behaviour: behaviour::moving,
            floor: 2,
            direction: Direction::up,
            cabRequests: vec!(false,false,true,true),
        };

        let id2 = TestState {
            behaviour: behaviour::idle,
            floor: 0,
            direction: Direction::stop,
            cabRequests: vec!(false,false,false,false),
        };

        let mut states = HashMap::new();
        states.insert("one".to_string(), id1);
        states.insert("two".to_string(), id2);

        let msg = Message {
            hallRequests: vec!([false,false],[true,false],[false,false],[false,true]),
            states: states,
        };

        let json_str = serde_json::to_string_pretty(&msg).unwrap();
        println!("Message: {}", json_str);


        let mut child = Command::new("./hall_request_assigner")
            .arg("--input")
            .arg(&json_str)
            .arg("--includeCab")
            .stdin(Stdio::piped()) 
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn().unwrap();

        let output = child.wait_with_output().await.unwrap();
        let formattedoutput = String::from_utf8_lossy(&output.stdout);
        let prettyoutput: String = serde_json::to_string_pretty(&formattedoutput).unwrap();


        println!("Stdout: {}", prettyoutput);
        //println!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
}


