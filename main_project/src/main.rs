use crate::networkhandler::Heartbeat;
use serde::Serialize;

use serde_json::Result;
use std::collections::HashMap;
use tokio::process::Command;
use std::process::Stdio;

mod config;
mod fsm;
mod types;
mod networkhandler;

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




#[tokio::main]
async fn main() {
    println!("Main started");
    // let mut elevator1 = fsm::ElevatorFSM::new("localhost:15657").await;
    // elevator1.transitions(fsm::Event::NewOrder(1)).await;
    // elevator1.transitions(fsm::Event::NewOrder(1)).await;
    // elevator1.transitions(fsm::Event::ArrivedAtFloor).await;

    // let mut network = networkhandler::Heartbeat::new().await;
    // loop {
    //     network.network_controller().await;
    // }
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


    // fsm::fsm_go_to_floor(2, &elevator1).await;
}
