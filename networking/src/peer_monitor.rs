use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, info};

pub async fn handle_peer(mut stream: TcpStream, addr: SocketAddr) {
    let mut buf = vec![0u8; 4096];
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => {
                info!(peer = %addr, "disconnected");
                break;
            }
            Ok(n) => {
                debug!(peer = %addr, bytes = n, "received");
                // echo back — for flow measurement
                let _ = stream.write_all(&buf[..n]).await;
            }
            Err(e) => {
                debug!(peer = %addr, err = %e, "read error");
                break;
            }
        }
    }
}
