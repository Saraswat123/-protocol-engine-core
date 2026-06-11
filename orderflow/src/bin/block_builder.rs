/// Block builder binary: fetch pending txs from a node, build optimum block.
///
/// Usage:
///   ETHEREUM_RPC=http://localhost:8545 cargo run -p orderflow --bin block-builder
///   ETHEREUM_RPC=https://holesky.drpc.org cargo run -p orderflow --bin block-builder
use orderflow::{
    builder::Builder,
    mempool::Mempool,
    rpc::EthRpcClient,
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let endpoint = std::env::var("ETHEREUM_RPC")
        .unwrap_or_else(|_| "http://localhost:8545".to_string());

    let tx_limit: usize = std::env::var("TX_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);

    let gas_limit: u64 = std::env::var("GAS_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30_000_000);

    info!(endpoint, tx_limit, gas_limit, "block builder starting");

    let client = EthRpcClient::new(&endpoint);

    // fetch base fee
    let base_fee = match client.base_fee().await {
        Ok(bf) => {
            info!(base_fee_gwei = bf, "fetched base fee");
            bf
        }
        Err(e) => {
            error!("failed to fetch base fee: {e}");
            std::process::exit(1);
        }
    };

    // fetch pending txs
    let txs = match client.pending_txs(tx_limit).await {
        Ok(t) => t,
        Err(e) => {
            error!("failed to fetch pending txs: {e}");
            std::process::exit(1);
        }
    };

    info!(count = txs.len(), "pending txs fetched");

    // fill mempool
    let mut pool = Mempool::new(base_fee, tx_limit);
    for tx in txs {
        pool.insert(tx);
    }

    // build block
    let builder = Builder::new(1, gas_limit);
    let block = builder.build(&pool, base_fee);
    let next_base_fee = Builder::next_base_fee(&block);

    println!("╔══════════════════════════════════════╗");
    println!("║         Block Builder Output         ║");
    println!("╠══════════════════════════════════════╣");
    println!("  Transactions  : {}", block.transactions.len());
    println!("  Gas used      : {} / {} ({:.1}%)",
        block.gas_used, block.gas_limit,
        block.gas_used as f64 / block.gas_limit as f64 * 100.0
    );
    println!("  Base fee      : {} Gwei", block.base_fee);
    println!("  Next base fee : {} Gwei", next_base_fee);

    if !block.transactions.is_empty() {
        println!("╠══════════════════════════════════════╣");
        println!("  Top 5 txs by tip:");
        for tx in block.transactions.iter().take(5) {
            let tip = tx.effective_tip(base_fee);
            println!("    {} | tip={}gwei gas={}", hex::encode(&tx.hash[..4]), tip, tx.gas_limit);
        }
    }
    println!("╚══════════════════════════════════════╝");
}
