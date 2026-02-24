mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use types::*;

use tokio::time::{timeout, Duration};

use crate::config::NUM_FLOORS;

#[tokio::main]
async fn main() {
    println!("Main started");

    let mut elevator1 = ElevatorFSM::new("localhost:15657").await;
    elevator1.transitions(Event::NewOrder(1)).await;

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

    // First phase: collect heartbeats for 4 seconds
    gossip_heartbeats = collect_gossip_for_duration(&mut network, gossip_heartbeats, 4).await;
    println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);

    request_assigner.elect_master(gossip_heartbeats.clone(), &mut network).await;

    // Second phase: collect more heartbeats for 6 seconds
    gossip_heartbeats = collect_gossip_for_duration(&mut network, gossip_heartbeats, 6).await;
    println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);

    loop {
    // sender/mottar UDP
    network.network_controller().await;

    // snapshot av gossip (kan ta f.eks. 200–500ms hvis du vil, men du har collect_gossip_heartbeats)
    let gossip = network.collect_gossip_heartbeats().await;

    // (valgfritt) kjør election først, men la oss anta at denne noden er master i demo
    if matches!(request_assigner.role, Roles::Master) {
        request_assigner.build_message_from_gossip(&gossip, NUM_FLOORS as usize).await;

        let assignments = request_assigner.cost_function().await;

        println!("\n--- ASSIGNMENTS ---");
        for (id, orders) in assignments {
            print!("{}: ", id);
            for o in orders {
                print!("[f{} {:?}] ", o.floor, o.order_type);
            }
            println!();
        }
    }

    tokio::time::sleep(Duration::from_millis(300)).await;
}
}

async fn collect_gossip_for_duration(
    network: &mut Heartbeat,
    mut gossip_heartbeats: Vec<HeartbeatMSG>,
    duration_secs: u64,
) -> Vec<HeartbeatMSG> {
    let phase = async {
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
    timeout(Duration::from_secs(duration_secs), phase).await;
    gossip_heartbeats
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

    #[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::HashMap;

    #[test]
    fn message_serializes_to_expected_d_format() {
        // Bygg en Message som ligner README-eksempelet til D
        let mut states = HashMap::new();
        states.insert(
            "one".to_string(),
            ElevatorState {
                behaviour: Behaviour::Moving,
                floor: 2,
                direction: Direction::Up,
                cab_requests: vec![false, false, true, true],
            },
        );
        states.insert(
            "two".to_string(),
            ElevatorState {
                behaviour: Behaviour::Idle,
                floor: 0,
                direction: Direction::Stop,
                cab_requests: vec![false, false, false, false],
            },
        );

        let msg = Message {
            hall_requests: vec![
                [false, false],
                [true, false],
                [false, false],
                [false, true],
            ],
            states,
        };
        println!("TYPE = {}", std::any::type_name::<Message>());
        println!("JSON  = {}", serde_json::to_string(&msg).unwrap());

        let s = serde_json::to_string(&msg).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();

        // Sjekk feltnavn
        assert!(v.get("hallRequests").is_some());
        assert!(v.get("states").is_some());
        assert!(v.get("hall_requests").is_none()); // viktig

        // Sjekk enum-serialisering (case)
        let one = &v["states"]["one"];
        assert_eq!(one["behaviour"], "moving");
        assert_eq!(one["direction"], "up");

        // Sjekk cabRequests finnes og har riktig lengde
        assert!(one.get("cabRequests").is_some());
        assert_eq!(one["cabRequests"].as_array().unwrap().len(), 4);
    }
}
