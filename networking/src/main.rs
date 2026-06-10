pub mod flow_tracker;
pub mod metrics;
pub mod peer_monitor;

use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let listener = TcpListener::bind("0.0.0.0:9000").await?;
    info!("P2P flow monitor listening on :9000");

    let mut tracker = flow_tracker::FlowTracker::new();

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        info!(peer = %peer_addr, "new connection");
        tracker.record_connection(peer_addr);

        tokio::spawn(peer_monitor::handle_peer(stream, peer_addr));
    }
}
