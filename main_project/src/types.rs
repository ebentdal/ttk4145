use rust_fsm::*;

pub enum Input {
    NewTargetFloor(u8),
    CallButtonPressed(u8),
    CabButtonPressed(u8),
    DoorUpdate(bool),
    Obstructed(bool),
    FloorReached,
    Initialized,
}

pub enum State {
    Init,
    Idle,
    WorkingOrder,
    Crashed,
}

pub enum Output {
    SetMotor(u8),
    SetDoor(u8),
    SetLights(u8),
}

state_machine! {
    #[state_machine(
        input(crate::types::Input),
        state(crate::types::State),
        output(crate::types::Output)
    )]
    
    ElevatorFSM(Init)

    Init(Initialized) => Idle,

    Idle(NewTargetFloor) => WorkingOrder [SetMotor],

    WorkingOrder(FloorReached) => Idle [SetDoor],
}
