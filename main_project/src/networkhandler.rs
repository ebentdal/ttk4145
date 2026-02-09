use network_rust::udpnet;
use std::net;
use crossbeam_channel as cbc;
use tokio;

use crate::config::PEER_PORT;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum Roles {
    Master,
    Slave,
}

#[derive(Debug, Clone)]
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

     pub async fn send_heartbeat(&self){
        let (peer_tx_enable_tx, peer_tx_enable_rx) = cbc::unbounded::<bool>();
        let id = self.ID.clone();
        let _handler = tokio::spawn(async move {
            if udpnet::bcast::tx(MSG_PORT, Heartbeat).is_err() {
                panic!("Oh no, it crashed RIP");
            }
        });

     }

     pub async fn revceive_master_orders(&self){

     }

}