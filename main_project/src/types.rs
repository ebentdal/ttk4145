use crossbeam_channel as cbc;
use driver_rust::elevio::elev::Elevator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub floor: u8,
    pub order_type: ButtonType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ButtonType {
    CabCall,
    HallUp,
    HallDown,
}

pub struct ElevatorFSM {
    pub queue: Vec<Order>,
    pub fsm: Elevator,
    pub obstruction: bool,
    pub prev_floor: u8,
    pub elev_id: String,
    pub state: ElevState,
    pub last_received_msg_counter: i32,
}

// --- Shared types 

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Roles {
    Master,
    Slave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Behaviour {
    Idle,
    Moving,
    DoorOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

pub struct Heartbeat {
    pub msg: HeartbeatMSG,
    pub rx: cbc::Receiver<HeartbeatMSG>,
    pub tx: cbc::Sender<HeartbeatMSG>,
}

// --- Request assigner types ---

#[derive(Serialize)]
pub struct Message {
    pub hall_requests: Vec<[bool; 2]>,
    pub states: HashMap<String, ElevatorState>,
}

#[derive(Serialize)]

pub struct ElevatorState {
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    pub cab_requests: Vec<bool>,
}

pub struct RequestAssigner {
    pub message: Message,
    pub id: String,
    pub role: Roles,
}
