mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use std::sync::Arc;

use types::*;
use tokio::time::{Duration};


#[tokio::main]
async fn main() {
    println!("Main started");

    // wrap elevator in Arc<Mutex> so we can drive it concurrently with the
    // network/task loop without holding a mutable borrow for long periods.

    let elevator1 = Arc::new(ElevatorFSM::new("localhost:15657").await); //TODO: endre navn på elevator1
    // kick off an initial movement if desired, locking briefly
    {
        elevator1.transitions(Event::NewOrder(1)).await; //kjører heisen til første etajse
    }

    let message = Message {
        hallRequests: vec![[false, false]; 4],
        states: std::collections::HashMap::new(),
    };


    let mut network = Heartbeat::new().await;

    let mut request_assigner = RequestAssigner::new(
        network.id().to_string(),
        Roles::Slave,
        message,
    ).await;

    // let mut injected = false; //kun for ordre én gang
    network.msg.external_orders = vec![
                Order { floor: 2, order_type: ButtonType::HallUp },
            ];
        network.msg.counter += 1;

// spawn a task to handle the elevator queue continuously
    {
        let elevator_clone = elevator1.clone();
        tokio::spawn(async move {
            loop {
                // simply call run_queue; it will manage its own locks
                elevator_clone.run_queue().await;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
    }

    loop {
        network.network_controller().await;

        let gossip = network.collect_gossip_heartbeats().await;
        println!("[MAIN] gossip: {:#?}", gossip);

        // determine current master/slave role before acting
        request_assigner
            .elect_master(gossip.clone(), &mut network)
            .await;

        println!("[MAIN] my role = {:?}", request_assigner.role);

        match request_assigner.role {
            Roles::Master => {
                request_assigner.master(&gossip, &mut network, elevator1.clone()).await;
            }
            Roles::Slave => {
                request_assigner.slave(&gossip, &network, elevator1.clone()).await;
            }
        }

        // print queue size periodically
        {
            let q = elevator1.queue.lock().await;
            println!("[MAIN] queue length = {}", q.len());
        }

        // (previously we drove the elevator here, which blocked the entire loop)
        // the actual movement is handled by a background task started earlier
        // so nothing to do here.
    }
}







// async fn collect_gossip_for_duration(
//     network: &mut Heartbeat,
//     mut gossip_heartbeats: Vec<HeartbeatMSG>,
//     duration_secs: u64,
// ) -> Vec<HeartbeatMSG> {
//     let phase = async {
//         loop {
//             network.network_controller().await;
//             let new_gossip = network.collect_gossip_heartbeats().await;

//             for new_msg in new_gossip {
//                 if let Some(pos) =
//                     gossip_heartbeats.iter().position(|h| h.id == new_msg.id)
//                 {
//                     if new_msg.counter > gossip_heartbeats[pos].counter {
//                         gossip_heartbeats[pos] = new_msg;
//                     }
//                 } else {
//                     gossip_heartbeats.push(new_msg);
//                 }
//             }
//         }
//     };

//     timeout(Duration::from_secs(5), phase).await;
//     gossip_heartbeats
// }

// async fn send_order_to_other_computer(network: &mut Heartbeat) {
//     let test_queue_external = vec![
//         Order { floor: 2, order_type: ButtonType::CabCall },
//         Order { floor: 3, order_type: ButtonType::CabCall },
//     ];

//     network.msg.external_orders = test_queue_external;
//     network.msg.counter += 1;

//     let phase1 = async {
//         loop {
//             network.network_controller().await;
//         }
//     };
//     timeout(Duration::from_secs(6), phase1).await;

//     let test_queue_external2 = vec![
//         Order { floor: 2, order_type: ButtonType::HallUp },
//         Order { floor: 3, order_type: ButtonType::HallDown },
//     ];

//     network.msg.external_orders = test_queue_external2;
//     network.msg.counter += 1;

//     let phase2 = async {
//         loop {
//             network.network_controller().await;
//         }
//     };
//     timeout(Duration::from_secs(6), phase2).await;
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use serde_json::Value;
//     use std::collections::HashMap;

//     #[test]
//     fn message_serializes_to_expected_d_format() {
//         let mut states = HashMap::new();

//         states.insert(
//             "one".to_string(),
//             ElevatorState {
//                 behaviour: Behaviour::Moving,
//                 floor: 2,
//                 direction: Direction::Up,
//                 cab_requests: vec![false, false, true, true],
//             },
//         );

//         states.insert(
//             "two".to_string(),
//             ElevatorState {
//                 behaviour: Behaviour::Idle,
//                 floor: 0,
//                 direction: Direction::Stop,
//                 cab_requests: vec![false, false, false, false],
//             },
//         );

//         let msg = Message {
//             hall_requests: vec![
//                 [false, false],
//                 [true, false],
//                 [false, false],
//                 [false, true],
//             ],
//             states,
//         };

//         println!("TYPE = {}", std::any::type_name::<Message>());
//         println!("JSON  = {}", serde_json::to_string(&msg).unwrap());

//         let s = serde_json::to_string(&msg).unwrap();
//         let v: Value = serde_json::from_str(&s).unwrap();

//         assert!(v.get("hallRequests").is_some());
//         assert!(v.get("states").is_some());
//         assert!(v.get("hall_requests").is_none());

//         let one = &v["states"]["one"];
//         assert_eq!(one["behaviour"], "moving");
//         assert_eq!(one["direction"], "up");

//         assert!(one.get("cabRequests").is_some());
//         assert_eq!(one["cabRequests"].as_array().unwrap().len(), 4);
//     }
// }


// let mut gossip_heartbeats: Vec<HeartbeatMSG> = Vec::new();

//     // First phase
//     gossip_heartbeats =
//         collect_gossip_for_duration(&mut network, gossip_heartbeats, 4).await;
//     println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);

//     request_assigner
//         .elect_master(gossip_heartbeats.clone(), &mut network)
//         .await;

//     // Second phase
//     gossip_heartbeats =
//     collect_gossip_for_duration(&mut network, gossip_heartbeats, 6).await;
//     println!("Collected gossip_heartbeas {:#?}", gossip_heartbeats);