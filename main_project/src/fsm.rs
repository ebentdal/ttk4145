use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio;
use crate::config;

pub struct ElevatorController {
    pub current_floor: Option<u8>,
    pub target_floor: Option<u8>,
    pub queue: Vec<u8>,
    pub fsm: ElevatorFSM,
}


pub async fn init_fsm() -> (Elevator, ElevatorController) {
    println!("FSM initialized");
    let mut elevator = Elevator::init("localhost:15657", config::NUM_FLOORS).unwrap();
    
    loop {
        Elevator::motor_direction(&mut elevator, elevio::elev::DIRN_DOWN);
        match Elevator::floor_sensor(&elevator) {
            Some(floor) => {
                println!("Elevator is at floor: {}", floor);
                let mut controller = ElevatorController {
                    current_floor: Some(floor),
                    target_floor: None,
                    queue: Vec::new(),
                    fsm: ElevatorFSM::new(), 
                };
                Elevator::motor_direction(&mut elevator, elevio::elev::DIRN_STOP);
                controller.fsm.process_event(Input::Initialized).ok();
                return (elevator, controller);
            },
            None => {
                println!("Elevator is between floors");
            }
        }
    }
}

pub async fn fsm_go_to_floor(queue: Vec<Output>, elevator: &Elevator, controller: &ElevatorController)

pub async fn run_controller(elevator: &Elevator, controller: &ElevatorController) {
    match {

    }
}


pub async fn handle_outputs(
    outputs: Vec<Output>,
    elevator: &Elevator,
    controller: &mut ElevatorController,
)


// pub async fn fsm_go_to_floor(target_floor: u8, elevator: &Elevator) {
//     println!("FSM going to floor: {}", target_floor);
//     loop {
//         match Elevator::floor_sensor(&elevator){
//            Some(floor) => {
//                 println!("Elevator is at floor: {}", floor);
//                 if floor < target_floor{
//                     Elevator::motor_direction(elevator, elevio::elev::DIRN_UP);
//                 }else if floor > target_floor {
//                     Elevator::motor_direction(elevator, elevio::elev::DIRN_DOWN);
//                 }else if floor == target_floor {
//                     Elevator::motor_direction(elevator, elevio::elev::DIRN_STOP);
//                     return;
//                 }
//             },
//             None => {
//                 println!("Elevator is between floors");
//             } 
//         } 
//     }

// }

pub async fn set_door(elevator: &Elevator) {
    door_light();
}

