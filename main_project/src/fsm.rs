use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio;
use driver_rust::elevio::elev::{DIRN_DOWN, DIRN_STOP, DIRN_UP};
use std::time::Duration;
use tokio::time::sleep;
use crate::config;
use crate::types::{ElevatorFSM, ElevState, Event};

impl ElevatorFSM {
    pub async fn new(addr: &str) -> Self {
        let (fsm, current_floor, obstruction) = Self::init_fsm(addr).await;
        Self {
            queue: Vec::new(),
            fsm,
            obstruction,
            prev_floor: current_floor,
            elev_id: addr.to_string(),
            state: ElevState::Init,
        }
    }

    async fn init_fsm(addr: &str) -> (Elevator, u8, bool) {
        println!("FSM initialized");
        let mut elevator = Elevator::init(addr, config::NUM_FLOORS).unwrap();

        loop {
            Elevator::motor_direction(&mut elevator, elevio::elev::DIRN_DOWN);
            match Elevator::floor_sensor(&elevator) {
                Some(floor) => {
                    Elevator::motor_direction(&mut elevator, elevio::elev::DIRN_STOP);
                    let obstruction = Elevator::obstruction(&elevator);
                    return (elevator, floor, obstruction);
                }
                None => {
                    println!("Elevator is between floors");
                }
            }
        }
    }

    pub async fn transitions(&mut self, event: Event) {
        match (self.state, event) {
            (ElevState::Init, _) => self.state = ElevState::Idle,

            (ElevState::Idle, Event::NewOrder(floor)) => {
                self.go_to_floor(floor).await;
                self.state = ElevState::WorkingOrder;
            }

            (ElevState::WorkingOrder, Event::ArrivedAtFloor) => {
                self.arrived_at_floor().await;
                self.state = ElevState::Idle;
            }

            _ => return,
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
        while Elevator::obstruction(&self.fsm) {
            sleep(Duration::from_micros(40)).await;
        }
        sleep(Duration::from_secs(3)).await;
        Elevator::door_light(&self.fsm, false);
    }
}
