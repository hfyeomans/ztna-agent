//! Simple UDP Echo Server for E2E Testing
//! Task 004: E2E Relay Testing
//!
//! Echoes back any UDP packets received. Used to test the relay path.

use std::env;
use std::net::UdpSocket;

fn main() {
    let args: Vec<String> = env::args().collect();

    let port = if args.len() > 2 && args[1] == "--port" {
        args[2].parse().expect("Invalid port number")
    } else {
        9999u16
    };

    let addr = format!("0.0.0.0:{}", port);
    let socket = UdpSocket::bind(&addr).expect("Failed to bind socket");

    println!("UDP Echo Server listening on {}", addr);

    let mut buf = [0u8; 65535];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                println!("Received {} bytes from {}", len, src);

                // Echo back
                if let Err(e) = socket.send_to(&buf[..len], src) {
                    eprintln!("Failed to send response: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Receive error: {}", e);
            }
        }
    }
}
