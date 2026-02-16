mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use types::*;

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

    let request_assigner = RequestAssigner::new(
        "elevator1".to_string(),
        Roles::Master,
        message,
    ).await;

 
    let test_queue_external = vec![
        Order { floor: 2, order_type: ButtonType::CabCall },
        Order { floor: 0, order_type: ButtonType::CabCall },
        Order { floor: 1, order_type: ButtonType::CabCall },
    ];
    let test_queue_internal = vec![
        Order { floor: 1, order_type: ButtonType::CabCall },
    ];




    let mut fsm_handle = tokio::spawn(async move {
        elevator1.run_queue().await;
    });

    let mut network = Heartbeat::new().await;
    network.msg.external_orders = test_queue_external;
    loop {
        network.network_controller().await;
    }
}
