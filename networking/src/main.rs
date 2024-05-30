use aya::Bpf;
use aya::programs::Tc;
use tracing::info;

pub mod tcp_flow_tracker;
pub mod latency_probe;
pub mod metrics;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    info!("🚀 Loading eBPF programs for P2P flow analysis...");
    
    // Load the BPF program
    let mut bpf = Bpf::load(include_bytes!("../ebpf-programs/tc_flow.bpf.o"))?;
    
    // Attach to eth0 ingress
    let program: &mut Tc = bpf.program_mut("tc_flow").unwrap().try_into()?;
    program.load()?;
    program.attach("eth0", aya::programs::TcAttachType::Ingress)?;

    info!("✅ P2P Flow Tracker active on eth0. Monitoring TCP flows...");
    
    tokio::signal::ctrl_c().await?;
    Ok(())
}
