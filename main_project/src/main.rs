mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;
mod testfunctions;

use types::*;

use tokio::time::{timeout, Duration};

use testfunctions::collect_gossip;

#[tokio::main]
async fn main() {

    let (mut network, mut request_assigner) = init_elevator().await;
    


    let mut gossip_heartbeats: Vec<HeartbeatMSG> = Vec::new();

    gossip_heartbeats = collect_gossip(&mut network, gossip_heartbeats, 4).await;
    println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);

    request_assigner.elect_master(gossip_heartbeats.clone(), &mut network).await;

    gossip_heartbeats = collect_gossip(&mut network, gossip_heartbeats, 6).await;
    println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);
}



pub async fn init_elevator() -> (Heartbeat, RequestAssigner) {
    println!("Initializing elevator and network...");

    let mut elevator = ElevatorFSM::new("localhost:15657").await;
    elevator.transitions(Event::NewOrder(1)).await; 

    let message = Message {
        hall_requests: vec![[false, false]; 4],
        states: std::collections::HashMap::new(),
    };

    let mut network = Heartbeat::new().await;

    let mut request_assigner = RequestAssigner::new(
        network.id().to_string(),
        Roles::Slave,
        message,
    ).await;

    (network, request_assigner)
}