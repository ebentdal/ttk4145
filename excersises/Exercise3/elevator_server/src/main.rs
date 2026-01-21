use driver_rust::elevio::elev::{self, Elevator};
use tokio::sync::mpsc;
use tokio::task;

#[derive(Debug)]
enum Command {
    GoTo(u8),
    Shutdown,
}

fn init_blocking(addr: &str, num_floors: u8) -> Elevator {
    println!("Init of elevator at {}", addr);
    let elevator = Elevator::init(addr, num_floors).unwrap();
    loop {
        Elevator::motor_direction(&elevator, elev::DIRN_DOWN);
        match Elevator::floor_sensor(&elevator) {
            Some(floor) => {
                if floor == 0 {
                    println!("Arrived at bottom on {}", addr);
                    Elevator::motor_direction(&elevator, elev::DIRN_STOP);
                    return elevator;
                }
            }
            None => continue,
        }
    }
}

fn go_to_floor_blocking(goal_floor: u8, elev: &Elevator) {
    println!("Going to floor: {}", goal_floor);
    let dir = get_direction_blocking(goal_floor, elev);
    loop {
        Elevator::motor_direction(elev, dir);
        match Elevator::floor_sensor(elev) {
            Some(floor) if floor == goal_floor => {
                println!("Arrived at goal: {}", goal_floor);
                Elevator::motor_direction(elev, elev::DIRN_STOP);
                return;
            }
            _ => continue,
        }
    }
}

fn get_direction_blocking(goal_floor: u8, elev: &Elevator) -> u8 {
    match Elevator::floor_sensor(elev) {
        Some(floor) => {
            if floor > goal_floor {
                elev::DIRN_DOWN
            } else {
                elev::DIRN_UP
            }
        }
        None => elev::DIRN_DOWN,
    }
}

fn elevator_task(addr: &str, num_floors: u8, mut rx: mpsc::UnboundedReceiver<Command>) {
    let elev = init_blocking(addr, num_floors);

    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
            Command::GoTo(floor) => {
                go_to_floor_blocking(floor, &elev);
            }
            Command::Shutdown => {
                println!("Shutting down elevator at {}", addr);
                Elevator::motor_direction(&elev, elev::DIRN_STOP);
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx1, rx1) = mpsc::unbounded_channel::<Command>();
    let (tx2, rx2) = mpsc::unbounded_channel::<Command>();

    let handle1 = task::spawn_blocking(move || {
        elevator_task("localhost:15657", 4, rx1);
    });

    let handle2 = task::spawn_blocking(move || {
        elevator_task("localhost:15658", 4, rx2);
    });

    tx1.send(Command::GoTo(3)).unwrap();
    tx2.send(Command::GoTo(1)).unwrap();

    tx1.send(Command::Shutdown).unwrap();
    tx2.send(Command::Shutdown).unwrap();

    let _ = tokio::join!(handle1, handle2);
}
