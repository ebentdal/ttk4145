use rust_fsm::*;

pub enum Input {
    NewTargetFloor(u8),
    CallButtonPressed(u8),
    CabButtonPressed(u8),
    Obstructed(bool),
    FloorReached,
}

pub enum State {
    Init,
    Idle,
    WorkingOrder,
    Crashed,
}

pub enum Output {
    GoToFloor,
    SetMotor,
    SetDoor,
    SetLights,
}

