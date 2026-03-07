use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio::elev::{DIRN_DOWN, DIRN_STOP, DIRN_UP};
use std::time::Duration;
use tokio::time::sleep;
use crate::config::NUM_FLOORS;
use crate::types::*;
use strum::IntoEnumIterator;

impl ElevatorInner {
    async fn new(addr: &str) -> Self {
        let mut driver = Elevator::init(addr, NUM_FLOORS).unwrap();
        loop {
            Elevator::motor_direction(&mut driver, DIRN_DOWN);
            if let Some(floor) = Elevator::floor_sensor(&driver) {
                Elevator::motor_direction(&mut driver, DIRN_STOP);
                Elevator::floor_indicator(&mut driver, floor);
                return Self {
                    obstruction: Elevator::obstruction(&driver),
                    driver,
                    prev_floor: floor,
                    direction: DIRN_STOP,
                    elev_id: addr.to_string(),
                    state: ElevState::Init,
                    last_received_msg_counter: 0,
                    currently_serving: None,
                };
            }
        }
    }
}

impl ElevatorFSM {
    pub async fn new(addr: &str) -> Self {
        Self {
            queue: tokio::sync::Mutex::new(Vec::new()),
            inner: tokio::sync::Mutex::new(ElevatorInner::new(addr).await),
        }
    }

    // Replace the queue with new assignments, preserving the order
    // currently being executed so it doesn't vanish mid-movement.
    pub async fn replace_queue(&self, orders: &[Order]) {
        let serving = self.inner.lock().await.currently_serving.clone();
        let mut new_queue: Vec<Order> = orders.to_vec();
        if let Some(ref s) = serving {
            if !new_queue.contains(s) {
                new_queue.insert(0, s.clone());
            }
        }
        *self.queue.lock().await = new_queue;
    }

    pub async fn poll_buttons(&self) -> Vec<Order> {
        let inner = self.inner.lock().await;
        let mut pressed = Vec::new();
        for floor in 0..NUM_FLOORS {
            for button in ButtonType::iter() {
                if Elevator::call_button(&inner.driver, floor, button as u8) {
                    pressed.push(Order { floor, order_type: button });
                }
            }
        }
        pressed
    }


    fn get_next_order(queue: &[Order], current_floor: u8, direction: u8) -> u8 {
        let mut next = queue[0].floor;
        for order in queue {
            match direction {
                DIRN_UP if order.floor >= current_floor
                        && order.order_type != ButtonType::HallDown => {
                    if order.floor < next || next < current_floor {
                        next = order.floor;
                    }
                }
                DIRN_DOWN if order.floor <= current_floor
                          && order.order_type != ButtonType::HallUp => {
                    if order.floor > next || next > current_floor {
                        next = order.floor;
                    }
                }
                _ if direction == DIRN_STOP => {
                    let dist = |f: u8| (f as i16 - current_floor as i16).abs();
                    if dist(order.floor) < dist(next) {
                        next = order.floor;
                    }
                }
                _ => {}
            }
        }
        next
    }


    pub async fn process_next_order(&self) -> Option<Order> {
        loop {
            sleep(Duration::from_millis(100)).await;

            let mut inner = self.inner.lock().await;
            if let Some(floor) = Elevator::floor_sensor(&inner.driver) {
                inner.prev_floor = floor;
                Elevator::floor_indicator(&inner.driver, floor);
            }

            let mut queue = self.queue.lock().await;
            if queue.is_empty() {
                inner.direction = DIRN_STOP;
                inner.state = ElevState::Idle;
                inner.currently_serving = None;
                Elevator::motor_direction(&inner.driver, DIRN_STOP);
                return None;
            }

            let next = Self::get_next_order(&queue, inner.prev_floor, inner.direction);

            if inner.prev_floor == next {
                let travel_dir = inner.direction;
                inner.state = ElevState::WorkingOrder;
                Elevator::motor_direction(&inner.driver, DIRN_STOP);

                let pos = queue.iter().position(|o| {
                    o.floor == next && match travel_dir {
                        DIRN_UP => o.order_type != ButtonType::HallDown,
                        DIRN_DOWN => o.order_type != ButtonType::HallUp,
                        _ => true,
                    }
                }).or_else(|| queue.iter().position(|o| o.floor == next))?;
                let served = queue.remove(pos);
                inner.currently_serving = None;

                let has_orders_ahead = queue.iter().any(|o| match travel_dir {
                    DIRN_UP => o.floor > inner.prev_floor,
                    DIRN_DOWN => o.floor < inner.prev_floor,
                    _ => false,
                });
                inner.direction = if has_orders_ahead { travel_dir } else { DIRN_STOP };

                println!("[FSM] Served f{} {:?} | queue: {:?}",
                    served.floor, served.order_type,
                    queue.iter().map(|x| x.floor).collect::<Vec<_>>());

                drop(queue);
                drop(inner);
                self.open_door_and_wait().await;
                self.inner.lock().await.state = ElevState::Idle;
                return Some(served);
            }

            inner.state = ElevState::WorkingOrder;
            inner.currently_serving = queue.iter().find(|o| o.floor == next).cloned();
            if inner.prev_floor < next {
                inner.direction = DIRN_UP;
                Elevator::motor_direction(&inner.driver, DIRN_UP);
            } else {
                inner.direction = DIRN_DOWN;
                Elevator::motor_direction(&inner.driver, DIRN_DOWN);
            }
        }
    }

    pub async fn get_state(&self) -> (u8, u8, Behaviour) {
        let inner = self.inner.lock().await;
        let behaviour = match inner.state {
            ElevState::WorkingOrder if inner.direction == DIRN_STOP => Behaviour::DoorOpen,
            ElevState::WorkingOrder                                 => Behaviour::Moving,
            _                                                       => Behaviour::Idle,
        };
        (inner.prev_floor, inner.direction, behaviour)
    }

    pub async fn open_door_and_wait(&self) {
        let inner = self.inner.lock().await;
        Elevator::door_light(&inner.driver, true);
        drop(inner);
        sleep(Duration::from_secs(3)).await;
        loop {
            let inner = self.inner.lock().await;
            if !Elevator::obstruction(&inner.driver) {
                Elevator::door_light(&inner.driver, false);
                return;
            }
            drop(inner);
            sleep(Duration::from_millis(40)).await;
        }
    }

    pub async fn set_button_light(&self, external_orders: &[Order], internal_orders: &[Order]) {
        let inner = self.inner.lock().await;
        for floor in 0..NUM_FLOORS {
            for button in ButtonType::iter() {
                let active = external_orders.iter().chain(internal_orders)
                    .any(|o| o.floor == floor && o.order_type == button);
                Elevator::call_button_light(&inner.driver, floor, button as u8, active);
            }
        }
    }
}
