//! Elevator hardware control and order-processing FSM.
//!
//! `ElevatorGuard` is the public interface. It owns the physical driver,
//! tracks current floor/direction/behaviour, and runs orders from a shared queue.

use driver_rust::elevio::elev::Elevator;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use crate::config::{
    NUM_FLOORS, ORDER_TIMEOUT, OBSTRUCTION_TIMEOUT,
    MOTOR_POLL_INTERVAL, INIT_FLOOR_POLL_INTERVAL, DOOR_POLL_INTERVAL, TASK_POLL_INTERVAL,
};
use crate::types::*;
use strum::IntoEnumIterator;


impl ElevatorGuard {
    /// Initialize hardware: drive down until a known floor is found, then stop.
    pub async fn new(addr: &str) -> Self {
        let driver = Elevator::init(addr, NUM_FLOORS).expect("Failed to connect to elevator simulator");
        Elevator::motor_direction(&driver, Direction::Down as u8);
        loop {
            if let Some(floor) = Elevator::floor_sensor(&driver) {
                Elevator::motor_direction(&driver, Direction::Stop as u8);
                Elevator::floor_indicator(&driver, floor);

                for f in 0..NUM_FLOORS {
                    for btn in ButtonType::iter() {
                        Elevator::call_button_light(&driver, f, btn as u8, false);
                    }
                }

                while Elevator::obstruction(&driver) { sleep(MOTOR_POLL_INTERVAL).await; }
                Elevator::door_light(&driver, false);

                return Self {
                    state: Mutex::new(ElevatorFSM {
                        driver,
                        floor,
                        direction: Direction::Stop,
                        behaviour: Behaviour::Idle,
                        serving:   None,
                        last_served_hall: None,
                    }),
                    queue: Mutex::new(Vec::new()),
                };
            }
            sleep(INIT_FLOOR_POLL_INTERVAL).await;
        }
    }

    // --- Private hardware helpers ---

    /// Read the floor sensor and update floor indicator if a new floor is detected.
    fn read_floor(state: &mut ElevatorFSM) {
        if let Some(floor) = Elevator::floor_sensor(&state.driver) {
            if floor != state.floor {
                // Leaving a floor clears the last served hall order lock.
                state.last_served_hall = None;
            }
            state.floor = floor;
            Elevator::floor_indicator(&state.driver, floor);
        }
    }

    /// Stop the motor and mark the elevator as idle with no active order.
    fn stop(state: &mut ElevatorFSM) {
        state.direction = Direction::Stop;
        state.behaviour = Behaviour::Idle;
        state.serving   = None;
        Elevator::motor_direction(&state.driver, Direction::Stop as u8);
    }

    /// Start moving toward `target`, recording which queued order is being served.
    fn move_toward(state: &mut ElevatorFSM, target: u8, queue: &[Order]) {
        state.behaviour = Behaviour::Moving;
        state.serving   = None;
        for order in queue {
            if order.floor == target {
                state.serving = Some(order.clone());
                break;
            }
        }
        state.direction = if state.floor < target { Direction::Up } else { Direction::Down };
        Elevator::motor_direction(&state.driver, state.direction as u8);
    }

    /// Stop at the current floor and remove the matching order from the queue.
    /// Returns the served order, or None if no direction-compatible order exists here.
    fn serve_order(state: &mut ElevatorFSM, queue: &mut Vec<Order>) -> Option<Order> {
        let travel_dir = state.direction;
        Elevator::motor_direction(&state.driver, Direction::Stop as u8);

        // Find an order at the current floor compatible with travel direction.
        // When we just arrived to serve a hall order, do not immediately serve
        // the opposite hall direction at that same stop.
        let mut pos: Option<usize> = None;
        for (i, order) in queue.iter().enumerate() {
            if order.floor != state.floor { continue; }

            let compatible = match travel_dir {
                Direction::Up   => order.order_type != ButtonType::HallDown,
                Direction::Down => order.order_type != ButtonType::HallUp,
                Direction::Stop => {
                    if let Some((served_floor, served_btn)) = state.last_served_hall {
                        if served_floor == state.floor {
                            let opposite = match served_btn {
                                ButtonType::HallUp => ButtonType::HallDown,
                                ButtonType::HallDown => ButtonType::HallUp,
                                _ => ButtonType::CabCall,
                            };
                            if order.order_type == opposite {
                                false
                            } else {
                                true
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                }
            };
            if compatible { pos = Some(i); break; }
        }

        let pos = match pos {
            Some(p) => p,
            None    => return None,
        };

        let served = queue.remove(pos);
        state.serving = None;

        // Remember what we just served so we don't clear the opposite hall direction
        // during the same stop.
        state.last_served_hall = match served.order_type {
            ButtonType::HallUp | ButtonType::HallDown => Some((state.floor, served.order_type)),
            _ => None,
        };

        // Keep travel direction if orders remain ahead; otherwise stop.
        let mut has_orders_ahead = false;
        for order in queue.iter() {
            let ahead = match travel_dir {
                Direction::Up   => order.floor > state.floor,
                Direction::Down => order.floor < state.floor,
                Direction::Stop => false,
            };
            if ahead { has_orders_ahead = true; break; }
        }
        state.direction = if has_orders_ahead { travel_dir } else { Direction::Stop };

        Some(served)
    }

    /// SCAN-like next target: nearest order ahead in current direction,
    /// falls back to the closest order if none are direction-compatible.
    fn next_target(queue: &[Order], floor: u8, direction: Direction) -> u8 {
        let mut best_floor: Option<u8> = None;

        for order in queue {
            let ahead = match direction {
                Direction::Up   => order.floor >= floor && order.order_type != ButtonType::HallDown,
                Direction::Down => order.floor <= floor && order.order_type != ButtonType::HallUp,
                Direction::Stop => true,
            };
            if !ahead { continue; }

            let better = match best_floor {
                None       => true,
                Some(prev) => match direction {
                    Direction::Up   => order.floor < prev,
                    Direction::Down => order.floor > prev,
                    Direction::Stop => {
                        let d_new  = (order.floor as i16 - floor as i16).abs();
                        let d_prev = (prev         as i16 - floor as i16).abs();
                        d_new < d_prev
                    }
                },
            };
            if better { best_floor = Some(order.floor); }
        }

        if let Some(f) = best_floor { return f; }

        // Fallback: if we're already moving, continue in that direction to
        // reduce unnecessary back-and-forth (e.g., 3↓ and 2↓ while at floor 0).
        // Otherwise (stopped), choose the closest order.
        match direction {
            Direction::Up => queue.iter().map(|o| o.floor).max().unwrap_or(floor),
            Direction::Down => queue.iter().map(|o| o.floor).min().unwrap_or(floor),
            Direction::Stop => {
                let mut closest_floor: Option<u8> = None;
                for order in queue {
                    let d = (order.floor as i16 - floor as i16).abs();
                    let better = match closest_floor {
                        None       => true,
                        Some(prev) => d < (prev as i16 - floor as i16).abs(),
                    };
                    if better { closest_floor = Some(order.floor); }
                }
                closest_floor.unwrap_or(floor)
            }
        }
    }

    // --- Public async API ---

    /// Drive toward the next queued order, serving it when we arrive.
    /// Returns Completed(order), Empty (queue was empty), or Failed (timeout).
    pub async fn process_next_order(&self) -> OrderResult {
        let order_start = std::time::Instant::now();
        loop {
            sleep(MOTOR_POLL_INTERVAL).await;

            let mut state = self.state.lock().await;
            Self::read_floor(&mut state);

            let mut queue = self.queue.lock().await;
            if queue.is_empty() {
                Self::stop(&mut state);
                return OrderResult::Empty;
            }

            if order_start.elapsed() > ORDER_TIMEOUT {
                println!("[FSM] ORDER TIMEOUT — restarting");
                Self::stop(&mut state);
                return OrderResult::Failed;
            }

            let target = Self::next_target(&queue, state.floor, state.direction);

            if state.floor != target {
                Self::move_toward(&mut state, target, &queue);
                continue;
            }

            // At target floor — attempt to serve an order.
            let served = match Self::serve_order(&mut state, &mut queue) {
                Some(order) => order,
                None => {
                    // No direction-compatible order here; reset so next pick is closest.
                    state.direction = Direction::Stop;
                    continue;
                }
            };

            let remaining: Vec<u8> = queue.iter().map(|o| o.floor).collect();
            println!("[FSM] Served f{} {:?} | remaining: {:?}", served.floor, served.order_type, remaining);

            drop(queue);
            drop(state);
            if !self.open_door_and_wait().await {
                return OrderResult::Failed;
            }

            return OrderResult::Completed(served);
        }
    }

    /// Returns the current floor, direction, and behaviour for broadcasting.
    pub async fn get_state(&self) -> (u8, Direction, Behaviour) {
        let state = self.state.lock().await;
        (state.floor, state.direction, state.behaviour)
    }

    /// Open the door for 3 seconds, then wait for obstruction to clear.
    /// Returns false if the obstruction timeout is exceeded (triggers a restart).
    async fn open_door_and_wait(&self) -> bool {
        {
            let mut state = self.state.lock().await;
            Elevator::door_light(&state.driver, true);
            state.behaviour = Behaviour::DoorOpen;
        }
        sleep(Duration::from_secs(3)).await;
        let obstruction_start = std::time::Instant::now();
        loop {
            let mut state = self.state.lock().await;
            if !Elevator::obstruction(&state.driver) {
                Elevator::door_light(&state.driver, false);
                state.behaviour = Behaviour::Idle;
                return true;
            }
            drop(state);
            if obstruction_start.elapsed() > OBSTRUCTION_TIMEOUT {
                println!("[FSM] OBSTRUCTION TIMEOUT — restarting");
                return false;
            }
            sleep(DOOR_POLL_INTERVAL).await;
        }
    }

    /// Replace the queue with new orders, preserving any order currently being served.
    pub async fn set_queue(&self, orders: &[Order]) {
        let serving = self.state.lock().await.serving.clone();

        let mut new_queue: Vec<Order> = orders.to_vec();
        if let Some(current) = serving {
            if !new_queue.contains(&current) {
                new_queue.insert(0, current);
            }
        }

        let mut q = self.queue.lock().await;
        if *q != new_queue {
            *q = new_queue;
        }
    }

    /// Read all currently pressed buttons and return them as orders.
    pub async fn poll_buttons(&self) -> Vec<Order> {
        let state = self.state.lock().await;
        let mut pressed = Vec::new();
        for floor in 0..NUM_FLOORS {
            for button in ButtonType::iter() {
                if Elevator::call_button(&state.driver, floor, button as u8) {
                    pressed.push(Order { floor, order_type: button });
                }
            }
        }
        pressed
    }

    /// Update all button lights to reflect the current set of active orders.
    pub async fn set_button_light(&self, hall_orders: &[Order], cab_orders: &[Order]) {
        let state = self.state.lock().await;
        for floor in 0..NUM_FLOORS {
            for button in ButtonType::iter() {
                let mut active = false;
                for order in hall_orders.iter().chain(cab_orders) {
                    if order.floor == floor && order.order_type == button {
                        active = true;
                        break;
                    }
                }
                Elevator::call_button_light(&state.driver, floor, button as u8, active);
            }
        }
    }

    /// Spawn order-runner and button-poller background tasks.
    /// Returns (completed_orders, button_presses, failure_signal) receivers.
    pub fn spawn_tasks(self: Arc<Self>) -> (
        tokio::sync::mpsc::UnboundedReceiver<Order>,
        tokio::sync::mpsc::UnboundedReceiver<Vec<Order>>,
        tokio::sync::mpsc::UnboundedReceiver<()>,
    ) {
        use tokio::sync::mpsc;
        let (completed_tx, completed_rx) = mpsc::unbounded_channel::<Order>();
        let (button_tx,    button_rx)    = mpsc::unbounded_channel::<Vec<Order>>();
        let (fail_tx,      fail_rx)      = mpsc::unbounded_channel::<()>();

        tokio::spawn({
            let elev = Arc::clone(&self);
            async move {
                loop {
                    match elev.process_next_order().await {
                        OrderResult::Completed(order) => { let _ = completed_tx.send(order); }
                        OrderResult::Failed           => { let _ = fail_tx.send(()); return; }
                        OrderResult::Empty            => {}
                    }
                    sleep(TASK_POLL_INTERVAL).await;
                }
            }
        });

        tokio::spawn(async move {
            loop {
                let pressed = self.poll_buttons().await;
                if !pressed.is_empty() { let _ = button_tx.send(pressed); }
                sleep(TASK_POLL_INTERVAL).await;
            }
        });

        (completed_rx, button_rx, fail_rx)
    }

    /// Cut motor power immediately. Used before restarting on failure.
    pub async fn emergency_stop(&self) {
        let state = self.state.lock().await;
        Elevator::motor_direction(&state.driver, Direction::Stop as u8);
    }
}
