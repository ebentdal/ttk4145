mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;
mod testfunctions;

use types::*;
use std::collections::HashMap;
use tokio::time::{timeout, Duration};
use testfunctions::collect_gossip;
use crate::testfunctions::send_order_to_other_computer;

#[tokio::main]
async fn main() {
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


    let mut gossip_heartbeats: Vec<HeartbeatMSG> = Vec::new();

    //send_order_to_other_computer(&mut network).await;

 


    gossip_heartbeats = collect_gossip(&mut network, gossip_heartbeats, 4).await;
    println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);

    request_assigner.elect_master(gossip_heartbeats.clone(), &mut network).await;

    let assignments: HashMap<String, Vec<Order>> = request_assigner.cost_function().await;

    println!("got assignments: {:#?}", assignments);

    network.msg.external_orders = vec![ Order { floor: 1, order_type: ButtonType::CabCall } ];
    network.msg.counter += 1;          // bump so receivers know it’s new
    network.network_controller().await; // sends the packet

    if let Some(hb) = network.network_controller().await {
        // received A’s heartbeat
        for order in hb.external_orders {
            println!("got order from A: floor {}", order.floor);
            // …push to FSM, etc…
        }
    }

    gossip_heartbeats = collect_gossip(&mut network, gossip_heartbeats, 6).await;
    println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);


    

}




