mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use types::*;

use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() {
    println!("Main started");

    let mut elevator1 = ElevatorFSM::new("localhost:15657").await;
    elevator1.transitions(Event::NewOrder(1)).await;
    // elevator1.transitions(Event::NewOrder(1)).await;
    // elevator1.transitions(Event::ArrivedAtFloor).await;

    let message = Message {
        hall_requests: vec![[false, false]; 4], // 4 floors, no hall orders
        states: std::collections::HashMap::new(),
    };

    let mut request_assigner = RequestAssigner::new(
        "elevator1".to_string(),
        Roles::Slave,
        message,
    ).await;

    let mut network = Heartbeat::new().await;

    let mut gossip_heartbeats: Vec<HeartbeatMSG> = Vec::new();

    let phase1 = async {
        loop {
            network.network_controller().await;
            let new_gossip = network.collect_gossip_heartbeats().await;
            
            // Merge new gossip with existing, updating only if counter is higher
            for new_msg in new_gossip {
                if let Some(pos) = gossip_heartbeats.iter().position(|h| h.id == new_msg.id) {
                    if new_msg.counter > gossip_heartbeats[pos].counter {
                        gossip_heartbeats[pos] = new_msg;
                    }
                } else {
                    gossip_heartbeats.push(new_msg);
                }
            }
        }
    };
    timeout(Duration::from_secs(6), phase1).await;

    

    println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);

    request_assigner.elect_master(gossip_heartbeats).await;

    //send_to_other_computer(&mut network).await;
    


}


async fn send_order_to_other_computer(network: &mut Heartbeat) {
    let test_queue_external= vec![
        Order { floor: 2, order_type: ButtonType::CabCall },
        Order { floor: 3, order_type: ButtonType::CabCall },
    ];
    
    network.msg.external_orders = test_queue_external;
    network.msg.counter += 1;

    let phase1 = async {
        loop {
            network.network_controller().await;
        }
    };
    timeout(Duration::from_secs(6), phase1).await;
    
    let test_queue_external2 = vec![
        Order { floor: 0, order_type: ButtonType::CabCall },
        Order { floor: 1, order_type: ButtonType::CabCall },
    ];

    network.msg.external_orders = test_queue_external2;
    network.msg.counter += 1;

    let phase2 = async {
            loop {
        network.network_controller().await;
        }
    };
    timeout(Duration::from_secs(6), phase2).await;
}