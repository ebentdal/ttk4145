use crossbeam_channel;
use driver_rust::elevio::elev::Elevator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{Instant, Duration};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;


// --- FSM types ---

#[derive(Copy, Clone, Debug)]
pub enum ElevState {
    Init,
    WorkingOrder,
    Crashed,
    Idle,
}

pub enum Event {
    NewOrder(u8),
    ArrivedAtFloor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Order {
    pub floor: u8,
    pub order_type: ButtonType,
}

#[repr(u8)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, EnumIter, Copy)]
pub enum ButtonType {
    CabCall = 2,
    HallUp = 0,
    HallDown = 1,
}

use tokio::sync::Mutex;

pub struct ElevatorInner {
    pub fsm: Elevator,
    pub obstruction: bool,
    pub prev_floor: u8,
    pub elev_id: String,
    pub state: ElevState,
    pub last_received_msg_counter: i32,
}

pub struct ElevatorFSM {
    pub queue: Mutex<Vec<Order>>,
    pub inner: Mutex<ElevatorInner>,
}

// --- Shared types 

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Roles {
    Master,
    Slave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Behaviour {
    idle, 
    moving,
    doorOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
    Stop,
}

// --- Network types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMSG {
    pub id: String,
    pub external_orders: Vec<Order>,
    pub internal_orders: Vec<Order>,
    pub floor: u8,
    pub direction: u8,
    pub status: Behaviour,
    pub counter: i32,
    pub role: Roles,
    pub assignments: std::collections::HashMap<String, Vec<Order>>, 
}

pub struct Heartbeat {
    pub msg: HeartbeatMSG,
    pub rx: tokio::sync::broadcast::Receiver<HeartbeatMSG>,
    pub tx_broadcast: tokio::sync::broadcast::Sender<HeartbeatMSG>,
    pub tx_udp: crossbeam_channel::Sender<HeartbeatMSG>,
}

// --- Request assigner types ---

#[derive(Serialize)]
pub struct Message {
    pub hallRequests: Vec<[bool; 2]>,
    pub states: HashMap<String, ElevatorState>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElevatorState {
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    pub cabRequests: Vec<bool>, // blir "cabRequests" utad
}

pub struct RequestAssigner {
    pub message: Message,
    pub id: String,
    pub role: Roles,

    pub last_published_assignments: HashMap<String, Vec<Order>>, //kanskje unødvendig
    pub last_seen: HashMap<String, Instant>,
    pub peer_ttl: Duration, //disse to for å hindre master flickering
}