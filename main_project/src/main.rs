mod config;
mod fsm;
pub mod types;
mod networkhandler;
mod requests;

use std::sync::Arc;
use types::*;
use tokio::time::Duration;

fn elevator_addr() -> String {
    let port: u16 = std::env::args()
        .nth(1)
        .map(|s| s.parse().expect("Invalid port number"))
        .unwrap_or(config::ELEVATOR_PORT);
    format!("localhost:{}", port)
}

fn restart_self() -> ! {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().expect("Failed to get current executable");
    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("[RESTART] Re-executing {:?} with args {:?}", exe, args);
    std::thread::sleep(Duration::from_secs(2));
    let err = std::process::Command::new(exe).args(&args).exec();
    panic!("Failed to restart: {}", err);
}


#[tokio::main]
async fn main() {
    let addr = elevator_addr();
    println!("Connecting to elevator simulator at {}", addr);

    let fsm = Arc::new(ElevatorFSM::new(&addr).await);
    let mut network = Network::new().await;
    let mut assigner = RequestAssigner::new(network.id().to_string());

    // Cab order recovery: listen for one gossip round before broadcasting our
    // own state, so peers still hold our pre-crash cab orders.
    assigner.recover_cab_orders_from_gossip(&network.collect_gossip().await, &mut network);

    let (mut completed_rx, mut button_rx, mut fail_rx) = fsm.clone().spawn_tasks();

    loop {
        if fail_rx.try_recv().is_ok() {
            println!("[MAIN] Failure detected — stopping motor and restarting");
            fsm.emergency_stop().await;
            restart_self();
        }

        network.update_state(fsm.get_state().await);
        network.broadcast_state().await;

        let gossip = network.collect_gossip().await;

        assigner.clear_completed_orders_from_gossip(&gossip, &mut network);
        assigner.elect_master(gossip.clone(), &mut network).await;

        while let Ok(orders) = button_rx.try_recv() {
            for order in orders { network.add_order(order); }
        }

        while let Ok(order) = completed_rx.try_recv() {
            println!("[MAIN] Order completed: f{} {:?}", order.floor, order.order_type);
            network.order_completed(order);
        }

        network.merge_gossip_orders(&gossip);

        match assigner.role {
            Roles::Master => assigner.master(&gossip, &mut network, fsm.clone()).await,
            Roles::Slave  => assigner.slave(&gossip, &mut network, fsm.clone()).await,
        }

        network.tick_cleared_order();
    }
}
