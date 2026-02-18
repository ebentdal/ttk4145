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

    // Test elect_master
    test_elect_master().await;
}

async fn test_elect_master() {
    println!("\n=== Testing elect_master ===\n");

    // Test 1: No master exists yet - one should be elected (smallest ID)
    println!("Test 1: No master exists - elect by smallest ID");
    let mut request_assigner = RequestAssigner::new(
        "elevator1".to_string(),
        Roles::Slave,
        Message {
            hall_requests: vec![[false, false]; 4],
            states: std::collections::HashMap::new(),
        },
    ).await;

    let gossip_heartbeats = vec![
        HeartbeatMSG {
            id: "elevator3".to_string(),
            external_orders: Vec::new(),
            internal_orders: Vec::new(),
            floor: 0,
            direction: 0,
            status: Behaviour::Idle,
            counter: 0,
            role: Roles::Slave,
        },
        HeartbeatMSG {
            id: "elevator1".to_string(),
            external_orders: Vec::new(),
            internal_orders: Vec::new(),
            floor: 0,
            direction: 0,
            status: Behaviour::Idle,
            counter: 0,
            role: Roles::Slave,
        },
        HeartbeatMSG {
            id: "elevator2".to_string(),
            external_orders: Vec::new(),
            internal_orders: Vec::new(),
            floor: 0,
            direction: 0,
            status: Behaviour::Idle,
            counter: 0,
            role: Roles::Slave,
        },
    ];

    request_assigner.elect_master(gossip_heartbeats).await;
    println!("Result: request_assigner is now {:?}\n", match request_assigner.role {
        Roles::Master => "MASTER",
        Roles::Slave => "SLAVE",
    });

    // Test 2: Master already exists
    println!("Test 2: Master already exists - no change");
    let mut request_assigner2 = RequestAssigner::new(
        "elevator2".to_string(),
        Roles::Slave,
        Message {
            hall_requests: vec![[false, false]; 4],
            states: std::collections::HashMap::new(),
        },
    ).await;

    let gossip_heartbeats_with_master = vec![
        HeartbeatMSG {
            id: "elevator1".to_string(),
            external_orders: Vec::new(),
            internal_orders: Vec::new(),
            floor: 0,
            direction: 0,
            status: Behaviour::Idle,
            counter: 0,
            role: Roles::Master,  // Already a master
        },
        HeartbeatMSG {
            id: "elevator2".to_string(),
            external_orders: Vec::new(),
            internal_orders: Vec::new(),
            floor: 0,
            direction: 0,
            status: Behaviour::Idle,
            counter: 0,
            role: Roles::Slave,
        },
    ];

    request_assigner2.elect_master(gossip_heartbeats_with_master).await;
    println!("Result: request_assigner2 is now {:?}\n", match request_assigner2.role {
        Roles::Master => "MASTER",
        Roles::Slave => "SLAVE",
    });
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