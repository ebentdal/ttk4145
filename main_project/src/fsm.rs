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
    pub async fn new(addr: &str, completed_tx: tokio::sync::mpsc::UnboundedSender<Order>) -> Self {
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
                    }),
                    queue: Mutex::new(Vec::new()),
                    completed_tx,
                };
            }
            sleep(INIT_FLOOR_POLL_INTERVAL).await;
        }
    }

    fn read_floor(state: &mut ElevatorFSM) {
        if let Some(floor) = Elevator::floor_sensor(&state.driver) {
            state.floor = floor;
            Elevator::floor_indicator(&state.driver, floor);
        }
    }

    fn stop(state: &mut ElevatorFSM) {
        state.direction = Direction::Stop;
        state.behaviour = Behaviour::Idle;
        state.serving   = None;
        Elevator::motor_direction(&state.driver, Direction::Stop as u8);
    }

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

    fn serve_order(state: &mut ElevatorFSM, queue: &mut Vec<Order>) -> Option<Order> {
        let travel_dir = state.direction;
        Elevator::motor_direction(&state.driver, Direction::Stop as u8);

        let mut pos: Option<usize> = None;
        for (i, order) in queue.iter().enumerate() {
            if order.floor != state.floor { continue; }
            let compatible = match travel_dir {
                Direction::Up   => order.order_type != ButtonType::HallDown,
                Direction::Down => order.order_type != ButtonType::HallUp,
                Direction::Stop => true,
            };
            if compatible { pos = Some(i); break; }
        }

        let pos = match pos {
            Some(p) => p,
            None    => return None,
        };

        let served = queue.remove(pos);
        state.serving = None;

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

            let served = match Self::serve_order(&mut state, &mut queue) {
                Some(order) => order,
                None => {
                    state.direction = Direction::Stop;
                    continue;
                }
            };

            let remaining: Vec<u8> = queue.iter().map(|o| o.floor).collect();
            println!("[FSM] Served f{} {:?} | remaining: {:?}", served.floor, served.order_type, remaining);

            self.completed_tx.send(served.clone()).unwrap();

            drop(queue);
            drop(state);
            if !self.open_door_and_wait().await {
                return OrderResult::Failed;
            }

            return OrderResult::Completed(served);
        }
    }

    pub async fn get_state(&self) -> (u8, Direction, Behaviour) {
        let state = self.state.lock().await;
        (state.floor, state.direction, state.behaviour)
    }

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

    pub async fn poll_buttons(&self) -> Vec<Order> {
        let state = self.state.lock().await;
        let mut pressed = Vec::new();
        for floor in 0..NUM_FLOORS {
            for button in ButtonType::iter() {
                if Elevator::call_button(&state.driver, floor, button as u8) {
                    if matches!(button, ButtonType::CabCall) && floor == state.floor && state.direction != Direction::Stop {
                        continue;
                    }
                    pressed.push(Order { floor, order_type: button });
                }
            }
        }
        pressed
    }

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

    pub fn spawn_tasks(self: Arc<Self>) -> (
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
                        OrderResult::Completed(_) => {}
                        OrderResult::Failed       => { let _ = fail_tx.send(()); return; }
                        OrderResult::Empty        => {}
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

        (button_rx, fail_rx)
    }

    pub async fn emergency_stop(&self) {
        let state = self.state.lock().await;
        Elevator::motor_direction(&state.driver, Direction::Stop as u8);
    }
}
