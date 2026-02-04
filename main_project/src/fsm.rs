use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio;
use driver_rust::elevio::poll::obstruction;
use crate::config;
use driver_rust::elevio::elev::{DIRN_DOWN, DIRN_STOP, DIRN_UP, CAB, HALL_UP, HALL_DOWN};
use std::time::Duration;
use tokio::time::sleep;

pub async fn init_fsm(addr: &str) -> (Elevator, u8, bool) {
    println!("FSM initialized");
    let mut elevator = Elevator::init(addr, config::NUM_FLOORS).unwrap();
    
    loop {
        Elevator::motor_direction(&mut elevator, elevio::elev::DIRN_DOWN);
        match Elevator::floor_sensor(&elevator) {
            Some(floor) => {
                Elevator::motor_direction(&mut elevator, elevio::elev::DIRN_STOP);
                let obstruction = Elevator::obstruction(&elevator);
                return (elevator, floor, obstruction);
            },
            None => {
                println!("Elevator is between floors");
            }
        }
    }
}

pub struct Order{
    pub floor: u8,
    pub order_type: ButtonType
}

pub enum ButtonType{
    CabCall,
    HallUp,
    HallDown
}
pub struct ElevatorFSM {
    queue: Vec<Order>,
    fsm: Elevator,
    obstruction: bool,
    prev_floor:u8,
    elev_id: String,
    state: State,
}

#[derive(Copy, Clone)]
pub enum State{
    Init,
    WorkingOrder,
    Crashed,
    Idle,
}

pub enum Event{
    NewOrder(u8),
    ArrivedAtFloor,
}

impl ElevatorFSM {
    pub async fn new(addr: &str) -> Self {
        let (fsm, current_floor, obstruction)  = init_fsm(addr).await;
        Self {
            queue: Vec::new(),
            fsm: fsm,
            obstruction: obstruction,
            prev_floor: current_floor,
            elev_id: addr.to_string(),
            state: State::Init,
        }
    }

    pub async fn transitions(&mut self, event: Event){
        match(self.state, event){
            
            (State::Init,_) => self.state = State::Idle,

            (State::Idle, Event::NewOrder(floor)) => { 
                self.go_to_floor(floor).await; 
                self.state = State::WorkingOrder;
            }

            (State::WorkingOrder, Event::ArrivedAtFloor) => {
                self.arrived_at_floor().await; 
                self.state = State::Idle;
            }

            _=> return
        }
    }

    pub async fn go_to_floor(&mut self, target_floor: u8) {
        println!("FSM going to floor: {}", target_floor);

        loop {
            match Elevator::floor_sensor(&self.fsm) {
                Some(floor) => {
                    println!("Elevator is at floor: {}", floor);

                    if floor < target_floor {
                        Elevator::motor_direction(&self.fsm, DIRN_UP);
                    } else if floor > target_floor {
                        Elevator::motor_direction(&self.fsm, DIRN_DOWN);
                    } else {
                        Elevator::motor_direction(&self.fsm, DIRN_STOP);
                        self.prev_floor = target_floor;
                        return;
                    }
                }
                None => {
                    println!("Elevator is between floors");
                }
            }
        }
    }

    pub async fn arrived_at_floor(&mut self) {
        Elevator::door_light(&self.fsm, true); 
        println!("Heil");
        while(Elevator::obstruction(&self.fsm)){};
        sleep(Duration::from_secs(3)).await;
        println!("Heil2");
        Elevator::door_light(&self.fsm, false);
    }

}

