use std::process::Command;
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout, Duration};

async fn primary() -> Result<(), std::io::Error> {
    let _ = spawn_backup();
    let socket = UdpSocket::bind("0.0.0.0:8080").await?;
    let mut counter = 0;

    loop {
        println!("Number: {}", counter);
        counter += 1;

        socket.send_to(b"alive", "0.0.0.0:8000").await?;

        sleep(Duration::from_millis(500)).await;
    }
    Ok(())
}

async fn run_backup() -> Result<(), std::io::Error> {
    let socket = UdpSocket::bind("0.0.0.0:8000").await?;

    let mut buf = [0u8; 1024];

    loop {
        let result = timeout(Duration::from_secs(2), socket.recv_from(&mut buf)).await;
        match result {
            Ok(_) => {
                //heartbeat
                continue;
            }
            Err(_) => {
                println!("Primary died, taking over...");
                drop(socket);
                primary().await?;
                break;
            }
        }
    }
    Ok(())
}

fn spawn_backup() -> Result<(), std::io::Error> {
    #[cfg(target_os = "linux")]
    Command::new("gnome-terminal")
        .args(&["--", "cargo", "run", "--", "backup"])
        .spawn()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    println!("Starting");

    let _ = run_backup().await;
    let _ = primary().await;

    Ok(())
}
