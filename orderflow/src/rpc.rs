/// JSON-RPC mempool fetcher: pulls pending txs from any Ethereum node
/// and converts them into our internal Transaction type.
use crate::mempool::Transaction;
use anyhow::{anyhow, Context};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info, warn};

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<Value>,
}

pub struct EthRpcClient {
    client: Client,
    endpoint: String,
}

impl EthRpcClient {
    pub fn new(endpoint: &str) -> Self {
        Self {
            client: Client::new(),
            endpoint: endpoint.to_string(),
        }
    }

    async fn call(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1,
        });

        let resp: RpcResponse = self
            .client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .context("RPC request failed")?
            .json()
            .await
            .context("RPC response parse failed")?;

        if let Some(err) = resp.error {
            return Err(anyhow!("RPC error: {err}"));
        }

        resp.result.ok_or_else(|| anyhow!("RPC returned null result"))
    }

    /// Fetch current base fee from the latest block (eth_getBlockByNumber).
    pub async fn base_fee(&self) -> anyhow::Result<u64> {
        let block = self.call("eth_getBlockByNumber", json!(["latest", false])).await?;
        let hex = block["baseFeePerGas"]
            .as_str()
            .ok_or_else(|| anyhow!("baseFeePerGas missing — pre-EIP-1559 node?"))?;
        let val = u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .context("baseFeePerGas parse")?;
        // convert wei → gwei
        Ok(val / 1_000_000_000)
    }

    /// Fetch pending transactions via txpool_content (Geth/Reth/Erigon).
    /// Returns up to `limit` transactions converted to our Transaction type.
    pub async fn pending_txs(&self, limit: usize) -> anyhow::Result<Vec<Transaction>> {
        let content = self.call("txpool_content", json!([])).await?;
        let pending = content["pending"]
            .as_object()
            .ok_or_else(|| anyhow!("txpool_content.pending missing"))?;

        let mut txs = Vec::new();

        'outer: for (sender_hex, nonce_map) in pending {
            let nonce_map = match nonce_map.as_object() {
                Some(m) => m,
                None => continue,
            };
            for (_nonce_str, tx_val) in nonce_map {
                if txs.len() >= limit {
                    break 'outer;
                }
                if let Some(tx) = parse_tx(sender_hex, tx_val) {
                    txs.push(tx);
                }
            }
        }

        info!(fetched = txs.len(), "pending txs from txpool");
        Ok(txs)
    }
}

fn parse_hex_u64(val: &Value) -> Option<u64> {
    u64::from_str_radix(val.as_str()?.trim_start_matches("0x"), 16).ok()
}

fn parse_hex_20(s: &str) -> [u8; 20] {
    let s = s.trim_start_matches("0x");
    let mut out = [0u8; 20];
    let bytes = hex::decode(s).unwrap_or_default();
    let len = bytes.len().min(20);
    out[..len].copy_from_slice(&bytes[..len]);
    out
}

fn parse_hex_32(val: &Value) -> [u8; 32] {
    let s = val.as_str().unwrap_or("").trim_start_matches("0x");
    let mut out = [0u8; 32];
    let bytes = hex::decode(s).unwrap_or_default();
    let len = bytes.len().min(32);
    out[..len].copy_from_slice(&bytes[..len]);
    out
}

fn parse_tx(sender_hex: &str, val: &Value) -> Option<Transaction> {
    let hash = parse_hex_32(&val["hash"]);
    let sender = parse_hex_20(sender_hex);
    let nonce = parse_hex_u64(&val["nonce"])?;
    let gas_limit = parse_hex_u64(&val["gas"])?;

    // EIP-1559 tx: use maxFeePerGas + maxPriorityFeePerGas
    // Legacy tx: use gasPrice for both
    let (gas_price, max_fee_per_gas) = if val["maxFeePerGas"].is_string() {
        let max = parse_hex_u64(&val["maxFeePerGas"])?;
        let prio = parse_hex_u64(&val["maxPriorityFeePerGas"]).unwrap_or(0);
        (prio / 1_000_000_000, max / 1_000_000_000)
    } else {
        let gp = parse_hex_u64(&val["gasPrice"])? / 1_000_000_000;
        (gp, gp)
    };

    let data = hex::decode(
        val["input"].as_str().unwrap_or("0x").trim_start_matches("0x")
    ).unwrap_or_default();

    debug!(hash = hex::encode(&hash[..4]), gas_price, max_fee_per_gas, "parsed tx");

    Some(Transaction { hash, sender, nonce, gas_price, max_fee_per_gas, gas_limit, data })
}
