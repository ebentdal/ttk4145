mod config;
mod fsm;

#[tokio::main]
async fn main() {
    println!("Main started");
    let mut elevator1 = fsm::init_fsm().await;
    fsm::fsm_go_to_floor(2, &elevator1).await;
}
