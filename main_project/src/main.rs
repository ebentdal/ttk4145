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

    let mut network: Heartbeat = Heartbeat::new().await;

    let message = Message {
        hall_requests: vec![[false, false]; 4],
        states: std::collections::HashMap::new(),
    };

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






    if !matches!(request_assigner.role, Roles::Master) {
        network.msg.external_orders = vec![
            Order { floor: 2, order_type: ButtonType::HallUp },
            Order { floor: 3, order_type: ButtonType::HallDown },
        ];
        network.msg.counter += 1;
        println!("Sent test external orders in heartbeat (counter={})", network.msg.counter);

        for _ in 0..20 {
            let _ = network.network_controller().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    } else {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }




    loop {
        let is_master = matches!(request_assigner.role, Roles::Master);

        if let Some(hb) = network.network_controller().await {
            let new_state = ElevatorState {
                behaviour: hb.status.clone(),
                floor: hb.floor,
                direction: match hb.direction {
                    0 => Direction::Stop,
                    1 => Direction::Up,
                    2 => Direction::Down,
                    _ => Direction::Stop,
                },
                cab_requests: hb
                    .internal_orders
                    .iter()
                    .map(|o| matches!(o.order_type, ButtonType::CabCall))
                    .collect(),
            };
            request_assigner.message.states.insert(hb.id.clone(), new_state);

            for order in hb.external_orders {
                if let ButtonType::HallUp | ButtonType::HallDown = order.order_type {
                    request_assigner.message.hall_requests[order.floor as usize][
                        if order.order_type == ButtonType::HallUp { 0 } else { 1 }
                    ] = true;
                } else {
                    elevator.queue.push(order);
                }
            }
        }

        if is_master {
            let assignments = request_assigner.cost_function().await;
            for (peer_id, orders) in &assignments {
                if peer_id == &request_assigner.id {
                    for o in orders {
                        elevator.queue.push(o.clone());
                    }
                } else if !orders.is_empty() {
                    network.msg.external_orders = orders.clone();
                    network.msg.counter += 1;
                }
            }
        }

        if !elevator.queue.is_empty() {
            elevator.run_queue().await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}




