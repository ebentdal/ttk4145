mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use std::sync::Arc;
use types::*;
use tokio::time::Duration;


// entry point of the elevator application.
// creates the FSM, network and request assigner, spawns a queue runner and
// then loops handling networking, elections, button presses, and queue state.
// returns `()` implicitly; invoked by the Tokio runtime.
#[tokio::main]
async fn main() {
    println!("Main started");

    let addr = format!("localhost:{}", config::ELEVATOR_PORT);
    println!("Connecting to elevator simulator at {}", addr);

    let fsm = Arc::new(ElevatorFSM::new(&addr).await);

    let message = Message {
        hall_requests: vec![[false, false]; config::NUM_FLOORS as usize],
        states: std::collections::HashMap::new(),
    };

    let mut network = Heartbeat::new().await;

    let mut request_assigner =
        RequestAssigner::new(network.id().to_string(), Roles::Slave, message);

    let (completed_tx, mut completed_rx) = tokio::sync::mpsc::unbounded_channel::<Order>();
    let (button_tx, mut button_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<Order>>();
    let mut clear_completed_after = None::<tokio::time::Instant>;

    tokio::spawn({
        let fsm = Arc::clone(&fsm);
        async move {
            loop {
                if let Some(order) = fsm.process_next_order().await {
                    let _ = completed_tx.send(order);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    });

    tokio::spawn({
        let fsm = Arc::clone(&fsm);
        async move {
            loop {
                let pressed = fsm.poll_buttons().await;
                if !pressed.is_empty() {
                    let _ = button_tx.send(pressed);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    });

    loop {
        let (floor, direction, status) = fsm.get_state().await;
        network.msg.floor = floor;
        network.msg.direction = direction;
        network.msg.status = status;

        // Process button presses and completed orders before the network
        // sleep so lights turn on within the current cycle, not the next.
        while let Ok(orders) = button_rx.try_recv() {
            for order in orders {
                let target = match order.order_type {
                    ButtonType::CabCall => &mut network.msg.internal_orders,
                    _ => &mut network.msg.external_orders,
                };
                if !target.contains(&order) {
                    target.push(order);
                    network.msg.counter += 1;
                }
            }
        }

        while let Ok(order) = completed_rx.try_recv() {
            println!("[MAIN] Order completed: f{} {:?}", order.floor, order.order_type);
            network.order_completed(order);
            clear_completed_after = Some(tokio::time::Instant::now() + Duration::from_secs(1));
        }

        // Run network send and gossip collection concurrently (~50 ms total
        // instead of ~70 ms sequentially).
        let gossip = {
            let (_, g) = tokio::join!(
                network.network_controller(),
                network.collect_gossip_heartbeats()
            );
            g
        };

        request_assigner.elect_master(gossip.clone(), &mut network).await;

        // Clear orders that other elevators have completed
        for heartbeat in &gossip {
            if let Some(cleared) = &heartbeat.cleared_order {
                network.msg.external_orders.retain(|o| o != cleared);
                network.msg.internal_orders.retain(|o| o != cleared);
            }
        }

        match request_assigner.role {
            Roles::Master => request_assigner.master(&gossip, &mut network, fsm.clone()).await,
            Roles::Slave  => request_assigner.slave(&gossip, &network, fsm.clone()).await,
        }

        {
            let q = fsm.queue.lock().await;
            let contents: Vec<String> = q.iter().map(|o| format!("f{} {:?}", o.floor, o.order_type)).collect();
            println!("[MAIN] queue ({}): [{}]", q.len(), contents.join(", "));
        }

        // Clear cleared_order after 1 second of broadcasting
        if let Some(clear_time) = clear_completed_after {
            if tokio::time::Instant::now() >= clear_time && network.msg.cleared_order.is_some() {
                network.msg.cleared_order = None;
                network.msg.counter += 1;
                clear_completed_after = None;
            }
        }
    }
}
