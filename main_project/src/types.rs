use crossbeam_channel;
use driver_rust::elevio::elev::Elevator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use strum_macros::EnumIter;


// --- FSM types ---

#[derive(Copy, Clone, Debug)]
pub enum ElevState {
    Init,
    WorkingOrder,
    Idle,
}

pub enum OrderResult {
    Completed(Order),
    Empty,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Order {
    pub floor: u8,
    pub order_type: ButtonType,
}

#[repr(u8)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, EnumIter, Copy)]
pub enum ButtonType {
    CabCall  = 2,
    HallUp   = 0,
    HallDown = 1,
}

use tokio::sync::Mutex;

pub struct ElevatorInner {
    pub driver: Elevator,
    pub last_floor: u8,
    pub direction: u8,
    pub state: ElevState,
    pub currently_serving: Option<Order>,
}

pub struct ElevatorFSM {
    pub queue: Mutex<Vec<Order>>,
    pub inner: Mutex<ElevatorInner>,
}


// --- Shared types ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Roles {
    Master,
    Slave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Behaviour {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "moving")]
    Moving,
    #[serde(rename = "doorOpen")]
    DoorOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
    Stop,
}


// --- Network types ---

/// The state message broadcast to all peers each network tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipMsg {
    pub id: String,
    pub hall_orders: Vec<Order>,
    pub cab_orders: Vec<Order>,
    pub floor: u8,
    pub direction: u8,
    pub behaviour: Behaviour,
    pub counter: i32,
    pub role: Roles,
    pub assignments: HashMap<String, Vec<Order>>,
    /// Each elevator's cab orders, gossiped across peers for crash recovery.
    pub peer_cab_orders: HashMap<String, Vec<Order>>,
    #[serde(rename = "clearedOrder")]
    pub cleared_order: Option<Order>,
}

/// Manages UDP broadcast communication with peers.
pub struct Network {
    /// Our own state, broadcast to peers every tick.
    pub state: GossipMsg,
    /// Subscribe to this channel to receive messages from other peers.
    pub incoming: tokio::sync::broadcast::Sender<GossipMsg>,
    pub udp_tx: crossbeam_channel::Sender<GossipMsg>,
    /// When to stop broadcasting a cleared_order (1 s after completion).
    pub cleared_at: Option<Instant>,
}


// --- Request assigner types ---

/// Input to the external hall_request_assigner binary.
#[derive(Serialize)]
pub struct Message {
    #[serde(rename = "hallRequests")]
    pub hall_requests: Vec<[bool; 2]>,
    pub states: HashMap<String, ElevatorState>,
}

/// Per-elevator state sent to the hall_request_assigner binary.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElevatorState {
    pub behaviour: Behaviour,
    pub floor: u8,
    pub direction: Direction,
    #[serde(rename = "cabRequests")]
    pub cab_requests: Vec<bool>,
}

pub struct RequestAssigner {
    pub message: Message,
    pub id: String,
    pub role: Roles,
    pub last_published_assignments: HashMap<String, Vec<Order>>,
    pub last_seen: HashMap<String, Instant>,
    /// Most recently received state for each live peer.
    pub cached_peers: HashMap<String, GossipMsg>,
    pub peer_ttl: Duration,
}
