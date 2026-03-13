pub const NUM_FLOORS:    u8  = 4;
pub const MSG_PORT:      u16 = 20009;
pub const ELEVATOR_PORT: u16 = 15657;

pub const MASTER_ELECTION_TIMEOUT: tokio::time::Duration  = tokio::time::Duration::from_millis(2000);
pub const OBSTRUCTION_TIMEOUT:     std::time::Duration    = std::time::Duration::from_secs(3);
pub const ORDER_TIMEOUT:           std::time::Duration    = std::time::Duration::from_secs(9);

pub const MOTOR_POLL_INTERVAL:      std::time::Duration = std::time::Duration::from_millis(100);
pub const INIT_FLOOR_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(20);
pub const DOOR_POLL_INTERVAL:       std::time::Duration = std::time::Duration::from_millis(40);
pub const TASK_POLL_INTERVAL:       std::time::Duration = std::time::Duration::from_millis(50);
