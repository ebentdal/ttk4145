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

 
    
    let test_queue_internal = vec![
        Order { floor: 1, order_type: ButtonType::CabCall },
    ];




    

    let mut network = Heartbeat::new().await;
    loop {
        if let Some(msg_recieved) = network.network_controller().await {
            request_assigner.send_to_own_fsm(&mut elevator1, msg_recieved).await;
            elevator1.run_queue().await;
        }
    }
}
