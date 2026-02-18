use crate::types::{HeartbeatMSG, Heartbeat, Message, Order, ButtonType, RequestAssigner, Roles};
use tokio::time::{timeout, Duration};


pub async fn collect_gossip(
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


pub async fn send_order_to_other_computer(network: &mut Heartbeat) {
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