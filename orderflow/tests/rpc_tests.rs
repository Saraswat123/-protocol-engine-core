use orderflow::rpc::EthRpcClient;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};
use serde_json::json;

async fn mock_server() -> MockServer {
    MockServer::start().await
}

fn rpc_response(result: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": result
    }))
}

// ── base fee ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_base_fee_parse() {
    let server = mock_server().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(rpc_response(json!({
            "baseFeePerGas": "0x5F5E100",  // 100_000_000 wei = 0.1 Gwei → rounds to 0
            "number": "0x1234"
        })))
        .mount(&server)
        .await;

    let client = EthRpcClient::new(&server.uri());
    let base_fee = client.base_fee().await.expect("should parse base fee");
    // 100_000_000 / 1_000_000_000 = 0 (integer division, < 1 Gwei)
    assert_eq!(base_fee, 0);
}

#[tokio::test]
async fn test_base_fee_1_gwei() {
    let server = mock_server().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(rpc_response(json!({
            "baseFeePerGas": "0x3B9ACA00",  // 1_000_000_000 wei = 1 Gwei
            "number": "0x1"
        })))
        .mount(&server)
        .await;

    let client = EthRpcClient::new(&server.uri());
    assert_eq!(client.base_fee().await.unwrap(), 1);
}

#[tokio::test]
async fn test_base_fee_missing_field_errors() {
    let server = mock_server().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(rpc_response(json!({ "number": "0x1" }))) // no baseFeePerGas
        .mount(&server)
        .await;

    let client = EthRpcClient::new(&server.uri());
    assert!(client.base_fee().await.is_err());
}

// ── pending txs ───────────────────────────────────────────────────────────────

fn eip1559_tx(hash: &str, _sender: &str, nonce: &str) -> serde_json::Value {
    json!({
        "hash": hash,
        "nonce": nonce,
        "gas": "0x5208",          // 21_000
        "maxFeePerGas": "0x2540BE400",   // 10 Gwei
        "maxPriorityFeePerGas": "0x3B9ACA00", // 1 Gwei
        "input": "0x",
        "value": "0x0"
    })
}

fn legacy_tx(hash: &str) -> serde_json::Value {
    json!({
        "hash": hash,
        "nonce": "0x1",
        "gas": "0x5208",
        "gasPrice": "0x77359400",   // 2 Gwei
        "input": "0x",
        "value": "0x0"
    })
}

#[tokio::test]
async fn test_pending_txs_eip1559() {
    let server = mock_server().await;
    let sender = "0xabcdef1234567890abcdef1234567890abcdef12";
    let hash = "0x1111111111111111111111111111111111111111111111111111111111111111";

    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(rpc_response(json!({
            "pending": {
                sender: {
                    "0": eip1559_tx(hash, sender, "0x0")
                }
            },
            "queued": {}
        })))
        .mount(&server)
        .await;

    let client = EthRpcClient::new(&server.uri());
    let txs = client.pending_txs(100).await.expect("should fetch txs");
    assert_eq!(txs.len(), 1);
    assert_eq!(txs[0].gas_limit, 21_000);
    assert_eq!(txs[0].max_fee_per_gas, 10); // 10 Gwei
}

#[tokio::test]
async fn test_pending_txs_legacy() {
    let server = mock_server().await;
    let sender = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let hash = "0x2222222222222222222222222222222222222222222222222222222222222222";

    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(rpc_response(json!({
            "pending": {
                sender: { "1": legacy_tx(hash) }
            },
            "queued": {}
        })))
        .mount(&server)
        .await;

    let client = EthRpcClient::new(&server.uri());
    let txs = client.pending_txs(100).await.unwrap();
    assert_eq!(txs.len(), 1);
    assert_eq!(txs[0].gas_price, 2); // 2 Gwei
    assert_eq!(txs[0].max_fee_per_gas, 2);
}

#[tokio::test]
async fn test_pending_txs_limit_respected() {
    let server = mock_server().await;
    // build 10 txs across 2 senders
    let mut pending = serde_json::Map::new();
    for s in 0..2u8 {
        let sender = format!("0x{:040x}", s);
        let mut nonce_map = serde_json::Map::new();
        for n in 0..5u8 {
            let hash = format!("0x{:064x}", (s as u64 * 100 + n as u64));
            nonce_map.insert(
                n.to_string(),
                eip1559_tx(&hash, &sender, &format!("0x{n:x}")),
            );
        }
        pending.insert(sender, serde_json::Value::Object(nonce_map));
    }

    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(rpc_response(json!({ "pending": pending, "queued": {} })))
        .mount(&server)
        .await;

    let client = EthRpcClient::new(&server.uri());
    let txs = client.pending_txs(3).await.unwrap();
    assert!(txs.len() <= 3, "limit must be respected: got {}", txs.len());
}

#[tokio::test]
async fn test_pending_txs_empty_pool() {
    let server = mock_server().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(rpc_response(json!({ "pending": {}, "queued": {} })))
        .mount(&server)
        .await;

    let client = EthRpcClient::new(&server.uri());
    let txs = client.pending_txs(100).await.unwrap();
    assert_eq!(txs.len(), 0);
}

#[tokio::test]
async fn test_rpc_error_propagates() {
    let server = mock_server().await;
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32601, "message": "Method not found" }
        })))
        .mount(&server)
        .await;

    let client = EthRpcClient::new(&server.uri());
    assert!(client.pending_txs(100).await.is_err());
}
