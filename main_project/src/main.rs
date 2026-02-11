mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use types::{ElevatorFSM, Event, Heartbeat};

#[tokio::main]
async fn main() {
    println!("Main started");

    let mut elevator1 = ElevatorFSM::new("localhost:15657").await;
    elevator1.transitions(Event::NewOrder(1)).await;
    elevator1.transitions(Event::NewOrder(1)).await;
    elevator1.transitions(Event::ArrivedAtFloor).await;

    let mut network = Heartbeat::new().await;
    loop {
        network.network_controller().await;
    }
}
