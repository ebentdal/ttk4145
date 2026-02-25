use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio;
use driver_rust::elevio::elev::{DIRN_DOWN, DIRN_STOP, DIRN_UP};
use std::time::Duration;
use tokio::time::sleep;
use crate::config::{self, NUM_FLOORS};
use crate::types::*;
use strum::IntoEnumIterator;

impl ElevatorFSM {
    pub async fn new(addr: &str) -> Self {
        let (fsm, current_floor, obstruction) = Self::init_fsm(addr).await;
        let inner = ElevatorInner {
            fsm,
            obstruction,
            prev_floor: current_floor,
            elev_id: addr.to_string(),
            state: ElevState::Init,
            last_received_msg_counter: 0,
        };

        Self {
            queue: tokio::sync::Mutex::new(Vec::new()),
            inner: tokio::sync::Mutex::new(inner),
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
                    //println!("Elevator is between floors");
                }
            }
        }
    }

    pub async fn transitions(&self, event: Event) {
        let mut inner = self.inner.lock().await;
        match (inner.state, event) {
            (ElevState::Init, _) => inner.state = ElevState::Idle,

            (ElevState::Idle, Event::NewOrder(floor)) => {
                // release inner temporarily to avoid holding while moving
                drop(inner);
                self.go_to_floor(floor).await;
                let mut inner = self.inner.lock().await;
                inner.state = ElevState::WorkingOrder;
            }

            (ElevState::WorkingOrder, Event::ArrivedAtFloor) => {
                drop(inner);
                self.arrived_at_floor().await;
                let mut inner = self.inner.lock().await;
                inner.state = ElevState::Idle;
            }

            _ => return,
        }
    }
    
    pub async fn check_for_button_press(&self) -> Option<Vec<Order>> {
        let mut button_press = Vec::new();
        let inner = self.inner.lock().await;

        for floor in 0..NUM_FLOORS {
            for button in ButtonType::iter() {
                if Elevator::call_button(&inner.fsm, floor, button as u8) {
                    button_press.push(Order {
                    floor,
                    order_type: button,
                    });
                }
            }
        }
        drop(inner);
        return Some(button_press);
    }   


    pub async fn go_to_floor(&self, target_floor: u8) {
        let mut inner = self.inner.lock().await;

        loop {
            match Elevator::floor_sensor(&inner.fsm) {
                Some(floor) => {
                    if floor < target_floor {
                        Elevator::motor_direction(&inner.fsm, DIRN_UP);
                    } else if floor > target_floor {
                        Elevator::motor_direction(&inner.fsm, DIRN_DOWN);
                    } else {
                        Elevator::motor_direction(&inner.fsm, DIRN_STOP);
                        inner.prev_floor = target_floor;
                        return;
                    }
                }
                None => {
                    // between floors
                }
            }
            drop(inner); // release while sleeping so other parts can lock if needed
            sleep(Duration::from_millis(100)).await;
            inner = self.inner.lock().await;
        }
    }


     pub async fn run_queue(&self) {
        loop {
            // take next order out of queue without holding hardware lock
            let order = {
                let mut q = self.queue.lock().await;
                if q.is_empty() {
                    None
                } else {
                    Some(q.remove(0))
                }
            };

            if let Some(order) = order {
                println!("Processing order to floor {}", order.floor);
                self.transitions(Event::NewOrder(order.floor)).await;
                self.transitions(Event::ArrivedAtFloor).await;
                println!("Order completed");
            } else {
                break;
            }
        }
    }


    pub async fn arrived_at_floor(&self) {
        let mut inner = self.inner.lock().await;
        Elevator::door_light(&inner.fsm, true);
        while Elevator::obstruction(&inner.fsm) {
            drop(inner);
            sleep(Duration::from_micros(40)).await;
            inner = self.inner.lock().await;
        }
        drop(inner);
        sleep(Duration::from_secs(3)).await;
        let mut inner = self.inner.lock().await;
        Elevator::door_light(&inner.fsm, false);
    }
}