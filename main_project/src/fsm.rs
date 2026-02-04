use driver_rust::elevio::elev::Elevator;
use driver_rust::elevio;
use crate::config;
use crate::types::{ElevatorFSM, Input, Output};

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

pub async fn handle_inputs(
    input: Input,
    elevator: &mut Elevator,
    controller: &mut ElevatorController,
) {
    match input{
        Input::NewOrder(Some(2))=> {
            fsm_go_to_floor(, elevator)
        },
    }
}

pub async fn handle_outputs(
    output: Output,
    elevator: &mut Elevator,
    controller: &mut ElevatorController,
) {
    match output{
        Output::NewOrder(Some(2))=> {
            fsm_go_to_floor(, elevator)
        },
        Output::OpenDoor => {
            // TODO: Open door
        },
        Output::CloseDoor => {
            // TODO: Close door
        },
        Output::ClearRequestsAtFloor(floor) => {
            // TODO: Clear requests at floor
        },
    }
}


pub async fn fsm_go_to_floor(target_floor: u8, elevator: &Elevator) {
     println!("FSM going to floor: {}", target_floor);
     loop {
         match Elevator::floor_sensor(&elevator){
            Some(floor) => {
                 println!("Elevator is at floor: {}", floor);
                 if floor < target_floor{
                     Elevator::motor_direction(elevator, elevio::elev::DIRN_UP);
                 }else if floor > target_floor {
                     Elevator::motor_direction(elevator, elevio::elev::DIRN_DOWN);
                 }else if floor == target_floor {
                    Elevator::motor_direction(elevator, elevio::elev::DIRN_STOP);
                     return;
                 }
             },
             None => {
                 println!("Elevator is between floors");
             } 
         } 
     }

}

pub async fn set_door(elevator: &Elevator) {
    // TODO: Implement door light control
}

