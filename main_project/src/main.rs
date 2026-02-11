mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use types::{ElevatorFSM, Event, RequestAssigner, Roles, Message, Order, ButtonType, Heartbeat};

#[tokio::main]
async fn main() {
    println!("Main started");

    let mut elevator1 = ElevatorFSM::new("localhost:15657").await;
    // elevator1.transitions(Event::NewOrder(1)).await;
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

 
    let test_queue = vec![
        Order { floor: 2, order_type: ButtonType::CabCall },
        Order { floor: 0, order_type: ButtonType::CabCall },
        Order { floor: 3, order_type: ButtonType::CabCall },
    ];


    request_assigner.send_to_own_fsm(&mut elevator1, test_queue).await;

    let mut fsm_handle = tokio::spawn(async move {
        elevator1.run_queue().await;
    });

    let mut network = Heartbeat::new().await;
    loop {
        network.network_controller().await;
    }
}
