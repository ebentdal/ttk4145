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

    /// Drive toward `initial_target`, but re-read the front of the queue each
    /// tick and follow it if it has changed (cost function may redirect us).
    /// Returns the floor actually stopped at.
    pub async fn go_to_floor(&self, initial_target: u8) -> u8 {
        loop {
            // Determine current target: queue front if available, else original.
            let target = {
                let q = self.queue.lock().await;
                q.first().map(|o| o.floor).unwrap_or(initial_target)
            };

            let maybe_floor = {
                let mut inner = self.inner.lock().await;
                let f = Elevator::floor_sensor(&inner.driver);
                if let Some(floor) = f {
                    inner.prev_floor = floor;
                    Elevator::floor_indicator(&inner.driver, floor);
                    if floor == target {
                        inner.direction = DIRN_STOP;
                        Elevator::motor_direction(&inner.driver, DIRN_STOP);
                        return floor;
                    }
                    let dir = if floor < target { DIRN_UP } else { DIRN_DOWN };
                    inner.direction = dir;
                    Elevator::motor_direction(&inner.driver, dir);
                }
                f
            };

            let _ = maybe_floor; // sensor checked above
            sleep(Duration::from_millis(100)).await;
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
                let inner = self.inner.lock().await;
                println!("!!!!!!! [FSM {}] >> EXECUTING: f{} {:?} | remaining queue: [{}] !!!!!!!", inner.elev_id, o.floor, o.order_type, remaining.join(", "));
                drop(inner);
                Some(o)
            }
        };

        if let Some(order) = order {
            {
                let mut inner = self.inner.lock().await;
                inner.state = ElevState::WorkingOrder;
                inner.currently_serving = Some(order.clone());
            }

            let actual_floor = self.go_to_floor(order.floor).await;

            // If the cost function redirected us to a different floor mid-journey,
            // serve that order and leave the original target in the queue for next time.
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

                self.open_door_and_wait().await;
                println!("[FSM] << Done (redirected): f{} {:?}", intermediate.floor, intermediate.order_type);
                intermediate
            } else {
                self.open_door_and_wait().await;
                println!("[FSM] << Done: f{} {:?}", order.floor, order.order_type);
                order
            };

            {
                let mut inner = self.inner.lock().await;
                inner.state = ElevState::Idle;
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
