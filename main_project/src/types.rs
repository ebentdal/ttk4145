use rust_fsm::*;

pub enum Input {
    Initialized,
    FloorReached(u8),
    TargetSelected(u8),
    DoorTimeout,
}   

pub enum State {
    Init,
    Idle,
    Moving,
    DoorOpen,
}

pub enum Output {
    SetMotor(Direction),
    OpenDoor,
    CloseDoor,
    ClearRequestsAtFloor(u8),
}

state_machine! {
    #[state_machine(input(crate::types::Input),
                    state(crate::types::State),
                    output(crate::types::Output))]
    ElevatorFSM(Init)

    Init(Initialized) => Idle;

    Idle(TargetSelected(_)) => MovingUp   [SetMotor(Direction::Up)];
    Idle(TargetSelected(_)) => MovingDown [SetMotor(Direction::Down)];

    MovingUp(FloorReached(_))   => DoorOpen [SetMotor(Direction::Stop), OpenDoor];
    MovingDown(FloorReached(_)) => DoorOpen [SetMotor(Direction::Stop), OpenDoor];

    DoorOpen(DoorTimeout) => Idle [CloseDoor];
}
