use crate::networkhandler::Heartbeat;
use serde::Serialize;

mod config;
mod fsm;
mod types;
mod networkhandler;
mod requests;



#[tokio::main]
async fn main() {
    println!("Main started");

    let mut elevator1 = fsm::ElevatorFSM::new("localhost:15657").await;
    elevator1.transitions(fsm::Event::NewOrder(1)).await;
    elevator1.transitions(fsm::Event::NewOrder(1)).await;
    elevator1.transitions(fsm::Event::ArrivedAtFloor).await;

    let mut network = networkhandler::Heartbeat::new().await;
    loop {
        network.network_controller().await;
    }


    // fsm::fsm_go_to_floor(2, &elevator1).await;
}
