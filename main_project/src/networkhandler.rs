use network_rust::udpnet;
use std::net;
use crossbeam_channel as cbc;
use tokio;
use serde::{Serialize, Deserialize};
use crate::config::MSG_PORT;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMSG{
    ID: String,
    ExternalOrders: Vec<u8>,
    InternalOrders: Vec<u8>,
    Floor: u8,
    Direction: u8,
    StatusFlag: Status,
    counter: i32,
    Role: Roles,
}

pub struct Heartbeat{
    HeartbeatMSG: HeartbeatMSG,
    RX: cbc::Receiver<HeartbeatMSG>,
    TX: cbc::Sender<HeartbeatMSG>,
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

        let (tx, rx) = Self::start_channels().await;

        let heartbeatmsg = HeartbeatMSG{
                ID: local_ip.to_string(),
                ExternalOrders: Vec::new(),
                InternalOrders: Vec::new(),
                Floor: 0,
                Direction: 0,
                StatusFlag: Status::Idle,
                counter: 0,
                Role: Roles::Slave,
        };

        Self {
                HeartbeatMSG: heartbeatmsg,
                TX: tx,
                RX: rx,
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
        
        return (bcast_tx, bcast_rx);
     }

     pub async fn network_controller(&mut self){
    
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        self.HeartbeatMSG.counter += 1; 
        self.TX.send(self.HeartbeatMSG.clone()).unwrap();
        
        println!("Sent heartbeat with counter: {}", self.HeartbeatMSG.counter);
        
        match self.RX.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(msg) => println!("received {:#?}", msg),
            Err(e) => println!("No message received: {:?}", e),
        }
     }

}