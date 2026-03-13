use network_rust::udpnet;
use std::{collections::HashMap, net::TcpStream};
use crossbeam_channel as cbc;
use tokio::sync::broadcast;
use crate::config::MSG_PORT;
use crate::types::*;


impl Network {

    pub async fn new() -> Self {
        let local_ip = TcpStream::connect("8.8.8.8:53")
            .unwrap()
            .local_addr()
            .unwrap()
            .ip();
        println!("Local IP: {}", local_ip);

        let (incoming, udp_tx) = Self::start_channels().await;
        let state = GossipMsg {
            id: local_ip.to_string(),
            hall_orders: Vec::new(),
            cab_orders: Vec::new(),
            floor: 0,
            direction: Direction::Stop,
            behaviour: Behaviour::Idle,
            counter: 0,
            role: Roles::Slave,
            assignments: HashMap::new(),
            peer_cab_orders: HashMap::new(),
            cleared_order: None,
        };

        Self { state, incoming, udp_tx, cleared_at: None }
    }

    async fn start_channels() -> (broadcast::Sender<GossipMsg>, cbc::Sender<GossipMsg>) {
        let (udp_send_tx, udp_send_rx) = cbc::unbounded::<GossipMsg>();
        let (udp_recv_tx, udp_recv_rx) = cbc::unbounded::<GossipMsg>();

        std::thread::spawn(move || {
            println!("[UDP] Starting RX on port {}", MSG_PORT);
            match udpnet::bcast::rx(MSG_PORT, udp_recv_tx) {
                Ok(_)  => println!("[UDP] RX done"),
                Err(e) => eprintln!("[UDP] RX failed: {:?}", e),
            }
        });

        std::thread::sleep(std::time::Duration::from_millis(100));

        std::thread::spawn(move || {
            println!("[UDP] Starting TX on port {}", MSG_PORT);
            match udpnet::bcast::tx(MSG_PORT, udp_send_rx) {
                Ok(_)  => println!("[UDP] TX done"),
                Err(e) => eprintln!("[UDP] TX failed: {:?}", e),
            }
        });

        let (relay_tx, _) = broadcast::channel::<GossipMsg>(512);
        let relay_tx_clone = relay_tx.clone();
        std::thread::spawn(move || {
            loop {
                match udp_recv_rx.recv() {
                    Ok(msg) => { let _ = relay_tx_clone.send(msg); }
                    Err(_)  => break,
                }
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        println!("Network channels initialized");

        (relay_tx, udp_send_tx)
    }

    pub async fn broadcast_state(&self) {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        self.udp_tx.send(self.state.clone()).expect("UDP TX channel closed unexpectedly");
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }


    pub async fn collect_gossip(&self) -> Vec<GossipMsg> {
        let mut by_id: HashMap<String, GossipMsg> = HashMap::new();
        let mut rx = self.incoming.subscribe();

        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);
        while std::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(10), rx.recv()).await {
                Ok(Ok(msg)) if msg.id != self.state.id => {
                    let better = by_id.get(&msg.id).map_or(true, |e| msg.counter > e.counter);
                    if better {
                        by_id.insert(msg.id.clone(), msg);
                    }
                }
                _ => {}
            }
        }

        by_id.into_values().collect()
    }


    pub fn id(&self) -> &str {
        &self.state.id
    }

    pub fn collect_cleared_orders(&self, gossip: &[GossipMsg]) -> Vec<Order> {
        let mut result = Vec::new();
        for p in std::iter::once(&self.state).chain(gossip.iter()) {
            if let Some(o) = &p.cleared_order {
                if !result.contains(o) {
                    result.push(o.clone());
                }
            }
        }
        result
    }

    pub fn order_completed(&mut self, order: Order) {
        self.state.hall_orders.retain(|o| o != &order);
        self.state.cab_orders.retain(|o| o != &order);
        for cabs in self.state.peer_cab_orders.values_mut() {
            cabs.retain(|o| o != &order);
        }
        self.state.cleared_order = Some(order);
        self.state.counter += 1;
        self.cleared_at = Some(tokio::time::Instant::now() + tokio::time::Duration::from_secs(1));
    }

    pub fn update_state(&mut self, (floor, direction, behaviour): (u8, Direction, Behaviour)) {
        self.state.floor = floor;
        self.state.direction = direction;
        self.state.behaviour = behaviour;
    }

    pub fn add_order(&mut self, order: Order) {
        let target = match order.order_type {
            ButtonType::CabCall => &mut self.state.cab_orders,
            _                   => &mut self.state.hall_orders,
        };
        if !target.contains(&order) {
            target.push(order);
            self.state.counter += 1;
        }
    }

    pub fn merge_gossip_orders(&mut self, gossip: &[GossipMsg]) {
        let cleared = self.collect_cleared_orders(gossip);
        for peer in gossip {
            for order in &peer.hall_orders {
                if !cleared.contains(order) && !self.state.hall_orders.contains(order) {
                    self.state.hall_orders.push(order.clone());
                }
            }
        }

        let my_id = self.state.id.clone();
        self.state.peer_cab_orders.insert(my_id.clone(), self.state.cab_orders.clone());
        for peer in gossip {
            for (id, cabs) in &peer.peer_cab_orders {
                if id == &my_id { continue; }
                let entry = self.state.peer_cab_orders.entry(id.clone()).or_default();
                for order in cabs {
                    if !entry.contains(order) {
                        entry.push(order.clone());
                    }
                }
            }
        }
    }

    pub fn tick_cleared_order(&mut self) {
        if let Some(expire_at) = self.cleared_at {
            if tokio::time::Instant::now() >= expire_at {
                self.state.cleared_order = None;
                self.state.counter += 1;
                self.cleared_at = None;
            }
        }
    }
}