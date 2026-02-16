use network_rust::udpnet;
use std::net;
use crossbeam_channel as cbc;
use crate::config::MSG_PORT;
use crate::types::*;
use rand::Rng;

impl Heartbeat {
    pub async fn new() -> Self {
        let local_ip = net::TcpStream::connect("8.8.8.8:53")
            .unwrap()
            .local_addr()
            .unwrap()
            .ip();
        println!("local ip {}", local_ip);

        let (tx, rx) = Self::start_channels().await;
        let mut rng = rand::thread_rng();
        let num = rng.gen_range(0..10000);
        let heartbeatmsg = HeartbeatMSG {
            id: num.to_string(),
            external_orders: Vec::new(),
            internal_orders: Vec::new(),
            floor: 0,
            direction: 0,
            status: Behaviour::Idle,
            counter: 0,
            role: Roles::Slave,
        };

        Self {
            msg: heartbeatmsg,
            tx,
            rx,
        }
    }

    pub async fn start_channels() -> (cbc::Sender<HeartbeatMSG>, cbc::Receiver<HeartbeatMSG>) {
        let (bcast_tx, bcast_tx_rx) = cbc::unbounded::<HeartbeatMSG>();
        tokio::spawn(async move {
            if udpnet::bcast::tx(MSG_PORT, bcast_tx_rx).is_err() {
                panic!("Broadcast TX failed");
            }
        });

        let (bcast_rx_tx, bcast_rx) = cbc::unbounded::<HeartbeatMSG>();
        tokio::spawn(async move {
            if udpnet::bcast::rx(MSG_PORT, bcast_rx_tx).is_err() {
                panic!("Broadcast RX failed");
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        println!("Network channels initialized");

        (bcast_tx, bcast_rx)
    }

    pub async fn network_controller(&mut self) -> Option<HeartbeatMSG> {
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    self.msg.counter += 1;
    self.tx.send(self.msg.clone()).unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    match self.rx.recv_timeout(std::time::Duration::from_millis(100)) {
        Ok(msg) if msg.id != self.msg.id => Some(msg),
        _ => None,
    }
}


    pub async fn send_heartbeat_to_request(&self) {
        //TODO send heartbeat message to requests.rs
    }

    pub fn floor(&self) -> u8 {
        self.msg.floor
    }

    pub fn direction(&self) -> u8 {
        self.msg.direction
    }

    pub fn status(&self) -> &Behaviour {
        &self.msg.status
    }

    pub fn id(&self) -> &str {
        &self.msg.id
    }

    pub fn internal_orders(&self) -> &Vec<Order> {
        &self.msg.internal_orders
    }
}
