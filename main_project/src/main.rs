mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use std::sync::Arc;
use types::*;
use tokio::time::{Duration};


// entry point of the elevator application.
// creates the FSM, network and request assigner, spawns a queue runner and
// then loops handling networking, elections, button presses, and queue state.
// returns `()` implicitly; invoked by the Tokio runtime.
#[tokio::main]
async fn main() {
    println!("Main started");

    let fsm = Arc::new(ElevatorFSM::new("localhost:5643").await); 

    {
        fsm.transitions(Event::NewOrder(1)).await; 
    }

    let message = Message {
        hallRequests: vec![[false, false]; 4],
        states: std::collections::HashMap::new(),
    };

    let mut network = Heartbeat::new().await;

    let mut request_assigner =
        RequestAssigner::new(network.id().to_string(), Roles::Slave, message).await;


    {
        tokio::spawn({
            let fsm = Arc::clone(&fsm);
            async move {
                loop {
                    fsm.run_queue().await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        });
    }

    loop {
        network.network_controller().await;

        let gossip = network.collect_gossip_heartbeats().await;
        println!("[MAIN] gossip: {:#?}", gossip);

        request_assigner
            .elect_master(gossip.clone(), &mut network)
            .await;

        println!("[MAIN] my role = {:?}", request_assigner.role);

        if let Some(orders) = fsm.check_for_button_press().await {
            if !orders.is_empty() {
                println!("[MAIN] button presses detected: {:?}", orders);

                let mut externalOrders = Vec::new();
                let mut internalOrders = Vec::new();
                for order in orders {
                    match order.order_type {
                        ButtonType::CabCall => internalOrders.push(order),
                        _ => externalOrders.push(order),
                    }
                }

                if !externalOrders.is_empty() {
                    network.msg.external_orders = externalOrders;
                }
                if !internalOrders.is_empty() {
                    // accumulate internal orders; we keep any previous ones too
                    network.msg.internal_orders.extend(internalOrders);
                }

                // bump counter whenever we added anything
                if !network.msg.external_orders.is_empty() || !network.msg.internal_orders.is_empty() {
                    network.msg.counter += 1;
                }
            }
        }

        match request_assigner.role {
            Roles::Master => {
                request_assigner.master(&gossip, &mut network, fsm.clone()).await;
            }
            Roles::Slave => {
                request_assigner.slave(&gossip, &network, fsm.clone()).await;
            }
        }

        {
            let q = fsm.queue.lock().await;
            println!("[MAIN] queue length = {}", q.len());
        }
    }
}
