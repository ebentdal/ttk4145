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

    let _request_assigner = RequestAssigner::new(
        "elevator1".to_string(),
        Roles::Master,
        message,
    ).await;


    let mut network = Heartbeat::new().await;
    send_to_other_computer(&mut network).await;
    


}


async fn send_to_other_computer(network: &mut Heartbeat) {
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

    loop {
        network.network_controller().await;
    }
}