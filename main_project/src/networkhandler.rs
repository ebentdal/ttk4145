use network_rust::udpnet;
use std::net;
use crossbeam_channel as cbc;
use tokio;
use serde::{Serialize, Deserialize};
use crate::config::{MSG_PORT, PEER_PORT};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat{
    ID: String,
    ExternalOrders: Vec<u8>,
    InternalOrders: Vec<u8>,
    Floor: u8,
    Direction: u8,
    StatusFlag: Status,
    counter: i32,
    Role: Roles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Roles {
    Master,
    Slave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Status {
    Working,
    Idle,
    OutOfOrder
}

impl Heartbeat {
     pub async fn new() -> Self { 
        let local_ip =  net::TcpStream::connect("8.8.8.8:53").unwrap()
             .local_addr()
             .unwrap()
             .ip();
        println!("local ip {}", local_ip);
        Self {
                ID: local_ip.to_string(),
                ExternalOrders: Vec::new(),
                InternalOrders: Vec::new(),
                Floor: 0,
                Direction: 0,
                StatusFlag: Status::Idle,
                counter: 0,
                Role: Roles::Slave,
            }
     }

     pub async fn send_heartbeat(&self) -> cbc::Sender<Heartbeat> {
        let (bcast_tx, bcast_rx) = cbc::unbounded::<Heartbeat>();
        let _handler = tokio::spawn(async move {
            if udpnet::bcast::tx(MSG_PORT, bcast_rx).is_err() {
                panic!("Broadcast transmit failed");
            }
        });
        bcast_tx
     }

     pub async fn network_controller(&self){
        let tx = self.send_heartbeat().await;
        tx.send(self.clone()).unwrap();
        let rx = self.receive_orders().await;  
        let msg = rx.recv().unwrap();
        println!("received {:#?}", msg);
     }

     pub async fn receive_orders(&self) -> cbc::Receiver<Heartbeat> {
        let (peer_update_tx, peer_update_rx) = cbc::unbounded::<Heartbeat>();
        {
            tokio::spawn(async move {
                if udpnet::bcast::rx(PEER_PORT, peer_update_tx).is_err() {
                    panic!("Oh no, i didnt receive the shit, or socket or wathereveradsdfefswfdsdr");
                }
            });
        }
        peer_update_rx
     }

}