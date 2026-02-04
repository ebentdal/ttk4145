use rust_fsm::*;

pub enum Input {
    Initialized,
    FloorReached(u8),
    TargetSelected(u8),
    DoorTimeout,
    Obstruction,  
    NewOrder(Option<u8>)
}   

pub enum State {
    Init,
    Idle,
    WorkingOrder,
    DoorOpen,
}

pub enum Output {
    SetMotor(Direction),
    OpenDoor,
    CloseDoor,
    ClearRequestsAtFloor(u8),
     NewOrder(Option<u8>)
}

state_machine! {
    #[state_machine(input(crate::types::Input),
                    state(crate::types::State),
                    output(crate::types::Output))]

    ElevatorFSM(Init)

    Init(Initialized) => Idle,

    Idle(NewOrder) => WorkingOrder [NewOrder],

    WorkingOrder(FloorReached(_)) => DoorOpen [SetMotor(Direction::Stop), OpenDoor],

    DoorOpen => {
        DoorTimeout => Idle [CloseDoor],
        Obstruction => DoorOpen,
    } 
}
