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

        // Drive down until a floor sensor is hit to establish a known position
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

    pub async fn handle_event(&self, event: Event) {
        let mut inner = self.inner.lock().await;
        match (inner.state, event) {
            (ElevState::Init, _) => inner.state = ElevState::Idle,

            (ElevState::Idle, Event::NewOrder(floor)) => {
                drop(inner);
                self.go_to_floor(floor).await;
                let mut inner = self.inner.lock().await;
                inner.state = ElevState::WorkingOrder;
            }

            (ElevState::WorkingOrder, Event::ArrivedAtFloor) => {
                drop(inner);
                self.open_door_and_wait().await;
                let mut inner = self.inner.lock().await;
                inner.state = ElevState::Idle;
            }

            _ => return,
        }
    }

    pub async fn poll_buttons(&self) -> Vec<Order> {
        let mut pressed = Vec::new();
        let inner = self.inner.lock().await;

        for floor in 0..NUM_FLOORS {
            for button in ButtonType::iter() {
                if Elevator::call_button(&inner.driver, floor, button as u8) {
                    pressed.push(Order { floor, order_type: button });
                }
            }
        }
        pressed
    }

    pub async fn go_to_floor(&self, target_floor: u8) -> u8 {
        let mut inner = self.inner.lock().await;

        loop {
            if let Some(floor) = Elevator::floor_sensor(&inner.driver) {
                inner.prev_floor = floor;
                Elevator::floor_indicator(&inner.driver, floor);

                // Check if there is a queued order at this floor (intermediate stop)
                drop(inner);
                let has_order_here = {
                    let q = self.queue.lock().await;
                    q.iter().any(|o| o.floor == floor)
                };
                inner = self.inner.lock().await;

                if floor == target_floor || has_order_here {
                    inner.direction = DIRN_STOP;
                    Elevator::motor_direction(&inner.driver, DIRN_STOP);
                    return floor;
                }

                if floor < target_floor {
                    inner.direction = DIRN_UP;
                    Elevator::motor_direction(&inner.driver, DIRN_UP);
                } else {
                    inner.direction = DIRN_DOWN;
                    Elevator::motor_direction(&inner.driver, DIRN_DOWN);
                }
            }
            drop(inner);
            sleep(Duration::from_millis(100)).await;
            inner = self.inner.lock().await;
        }
    }

    pub async fn get_state(&self) -> (u8, u8, Behaviour) {
        let inner = self.inner.lock().await;
        let behaviour = match inner.state {
            ElevState::WorkingOrder if inner.direction == DIRN_STOP => Behaviour::DoorOpen,
            ElevState::WorkingOrder => Behaviour::Moving,
            _ => Behaviour::Idle,
        };
        (inner.prev_floor, inner.direction, behaviour)
    }

    pub async fn process_next_order(&self) -> Option<Order> {
        let order = {
            let mut q = self.queue.lock().await;
            if q.is_empty() {
                None
            } else {
                let o = q.remove(0);
                let remaining: Vec<String> = q.iter().map(|x| format!("f{} {:?}", x.floor, x.order_type)).collect();
                println!("[FSM] >> Serving: f{} {:?} | queue: [{}]", o.floor, o.order_type, remaining.join(", "));
                Some(o)
            }
        };

        if let Some(order) = order {
            {
                let mut inner = self.inner.lock().await;
                inner.currently_serving = Some(order.clone());
            }
            self.handle_event(Event::NewOrder(order.floor)).await;

            // Read where we actually stopped - may differ from order.floor if the
            // cost function inserted a closer order into the queue mid-journey.
            let actual_floor = {
                let inner = self.inner.lock().await;
                inner.prev_floor
            };

            let served_order = if actual_floor != order.floor {
                // Intermediate stop: dequeue the order at this floor from the queue.
                let intermediate = {
                    let mut q = self.queue.lock().await;
                    if let Some(pos) = q.iter().position(|o| o.floor == actual_floor) {
                        q.remove(pos)
                    } else {
                        Order { floor: actual_floor, order_type: ButtonType::HallUp }
                    }
                };

                self.handle_event(Event::ArrivedAtFloor).await;

                // Re-insert the original target at the front so it is served next.
                {
                    let mut q = self.queue.lock().await;
                    if !q.contains(&order) {
                        q.insert(0, order.clone());
                    }
                }

                println!("[FSM] << Intermediate: f{} {:?} (original target f{})",
                    intermediate.floor, intermediate.order_type, order.floor);
                intermediate
            } else {
                self.handle_event(Event::ArrivedAtFloor).await;
                println!("[FSM] << Done:    f{} {:?}", order.floor, order.order_type);
                order
            };

            {
                let mut inner = self.inner.lock().await;
                inner.currently_serving = None;
            }
            return Some(served_order);
        }
        None
    }

    pub async fn open_door_and_wait(&self) {
        {
            let inner = self.inner.lock().await;
            Elevator::door_light(&inner.driver, true);
        }
        sleep(Duration::from_secs(3)).await;
        // After the mandatory 3-second open period, wait until obstruction clears
        loop {
            let inner = self.inner.lock().await;
            if !Elevator::obstruction(&inner.driver) {
                Elevator::door_light(&inner.driver, false);
                break;
            }
            drop(inner);
            sleep(Duration::from_millis(40)).await;
        }
    }

    pub async fn set_button_light(&self, external_orders: &[Order], internal_orders: &[Order]) {
        let inner = self.inner.lock().await;

        for floor in 0..NUM_FLOORS {
            for button in ButtonType::iter() {
                let active = external_orders.iter().chain(internal_orders.iter())
                    .any(|o| o.floor == floor && o.order_type == button);
                Elevator::call_button_light(&inner.driver, floor, button as u8, active);
            }
        }
    }
}
