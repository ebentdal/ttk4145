use std::net::{ UdpSocket, TcpStream, TcpListener};
use std::io::prelude::*;


fn rec_udp(socket_addr: &str) -> std::io::Result<()> {
    let socket = UdpSocket::bind(socket_addr)?;
    let mut buf = [0u8; 1024]; // Buffer for incoming datagrams
    let (amt, src_addr) = socket.recv_from(&mut buf)?;
    let received = String::from_utf8_lossy(&buf[..amt]);
    println!("Received {} bytes from {}: {:?}", 
                 amt, src_addr,received);           
    Ok(())
}

fn send_udp(target: &str) -> std::io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let data = b"Hi from SENDER";
    
    let sent = socket.send_to(data, target)?;
    println!("Sent {sent} bytes to {target}");
    Ok(())
}

fn connect_tcp(target: &str, msg: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(target)?;

    let mut buf = [0u8; 128];
    let mut total = Vec::new();
    
    // Keep reading until we get \0 OR connection closes
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 { break; }  // Server closed connection
        
        total.extend_from_slice(&buf[..n]);
        if total.contains(&0u8) { break; }  // Found null terminator
    }
    let len = total.iter().position(|&b| b == 0).unwrap_or(total.len());
    let received = std::str::from_utf8(&total[..len]).unwrap_or("invalid utf8");
    println!("Received: {}", received);


    stream.write_all(msg.as_bytes())?;

    let mut buf = [0u8; 128];
    let mut total = Vec::new();
    
    // Keep reading until we get \0 OR connection closes
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 { break; }  // Server closed connection
        
        total.extend_from_slice(&buf[..n]);
        if total.contains(&0u8) { break; }  // Found null terminator
    }
    let len = total.iter().position(|&b| b == 0).unwrap_or(total.len());
    let received = std::str::from_utf8(&total[..len]).unwrap_or("invalid utf8");
    println!("Received: {}", received);

    

    Ok(())
}

fn accept_tcp(target: &str, msg: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(target)?;

    let mut buf = [0u8; 128];
    let mut total = Vec::new();
    
    // Keep reading until we get \0 OR connection closes
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 { break; }  // Server closed connection
        
        total.extend_from_slice(&buf[..n]);
        if total.contains(&0u8) { break; }  // Found null terminator
    }
    let len = total.iter().position(|&b| b == 0).unwrap_or(total.len());
    let received = std::str::from_utf8(&total[..len]).unwrap_or("invalid utf8");
    println!("Received accept1: {}", received);

    let listener = TcpListener::bind(msg)?;
    println!("Listening on {}", msg);
    let tot_msg = String::from("Connect to: ") + msg + "\x00";
    stream.write_all(tot_msg.as_bytes())?;

    for stream in listener.incoming() {
        let mut stream = stream?;  //
        
        let mut buf = [0u8; 128];
        let mut total = Vec::new();

        // Read from STREAM, not listener
        loop {
            let n = stream.read(&mut buf)?;  //
            if n == 0 { break; }
            total.extend_from_slice(&buf[..n]);
            if total.contains(&0u8) { break; }
        }

        let len = total.iter().position(|&b| b == 0).unwrap_or(total.len());
        let received = std::str::from_utf8(&total[..len]).unwrap_or("invalid utf8");
        println!("Received accept last welcome: {}", received);

        stream.write_all(b"Niahao, shi daowei, accept tcp\x00")?;

        let mut buf = [0u8; 128];
        let mut total = Vec::new();

        // Read from STREAM, not listener
        loop {
            let n = stream.read(&mut buf)?;  //
            if n == 0 { break; }
            total.extend_from_slice(&buf[..n]);
            if total.contains(&0u8) { break; }
        }

        let len = total.iter().position(|&b| b == 0).unwrap_or(total.len());
        let received = std::str::from_utf8(&total[..len]).unwrap_or("invalid utf8");
        println!("Received accept last: {}", received);
        break;
    }





    Ok(())
}


fn main(){ 

    let _ = rec_udp("0.0.0.0:30000");
    let _ = send_udp("10.22.235.14:20000");
    let _ = rec_udp("10.22.235.14:20001"); 
    let _ = connect_tcp("10.22.235.14:33546", "nihao pengyou\x00");
    let _ = accept_tcp("10.22.235.14:33546", "10.22.235.14:35000");
}


