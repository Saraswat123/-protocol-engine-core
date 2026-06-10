use engine_sync::{EngineApi, EngineApiEvent, ExecutionPayload, ForkchoiceState, PayloadStatus};
use tokio::sync::mpsc;

fn genesis_hash() -> [u8; 32] { [0u8; 32] }

fn payload(number: u64, parent: [u8; 32], gas_used: u64) -> ExecutionPayload {
    let mut hash = [0u8; 32];
    hash[0] = (number & 0xff) as u8;
    hash[1] = 0xab;
    ExecutionPayload {
        block_hash: hash,
        parent_hash: parent,
        block_number: number,
        gas_limit: 30_000_000,
        gas_used,
        timestamp: number * 12,
        transactions: vec![],
    }
}

async fn engine() -> (EngineApi, mpsc::Receiver<EngineApiEvent>) {
    let (tx, rx) = mpsc::channel(64);
    (EngineApi::new(genesis_hash(), 30_000_000, tx), rx)
}

// ── newPayload ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_new_payload_valid() {
    let (mut api, mut rx) = engine().await;
    let p = payload(1, genesis_hash(), 21_000);
    let status = api.new_payload(p).await;
    assert_eq!(status, PayloadStatus::Valid);

    let event = rx.try_recv().unwrap();
    assert!(matches!(event, EngineApiEvent::NewPayloadReceived { status: PayloadStatus::Valid, .. }));
}

#[tokio::test]
async fn test_new_payload_gas_exceeded() {
    let (mut api, _rx) = engine().await;
    // gas_used > gas_limit
    let p = payload(1, genesis_hash(), 40_000_000);
    let status = api.new_payload(p).await;
    assert!(matches!(status, PayloadStatus::Invalid { .. }));
}

#[tokio::test]
async fn test_new_payload_unknown_parent_syncing() {
    let (mut api, _rx) = engine().await;
    let unknown_parent = [0xde; 32];
    let p = payload(5, unknown_parent, 0);
    let status = api.new_payload(p).await;
    assert_eq!(status, PayloadStatus::Syncing);
}

#[tokio::test]
async fn test_new_payload_wrong_gas_limit() {
    let (mut api, _rx) = engine().await;
    let mut p = payload(1, genesis_hash(), 0);
    p.gas_limit = 15_000_000; // doesn't match engine's 30M
    let status = api.new_payload(p).await;
    assert!(matches!(status, PayloadStatus::Invalid { .. }));
}

#[tokio::test]
async fn test_chain_of_payloads() {
    let (mut api, _rx) = engine().await;
    // import blocks 1..5 in order
    let mut parent = genesis_hash();
    for n in 1u64..=5 {
        let p = payload(n, parent, 21_000);
        let h = p.block_hash;
        let status = api.new_payload(p).await;
        assert_eq!(status, PayloadStatus::Valid, "block {n} must be valid");
        parent = h;
    }
    assert_eq!(api.exec.height, 5);
}

// ── forkchoiceUpdated ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_forkchoice_updated_known_head() {
    let (mut api, mut rx) = engine().await;
    // import block 1
    let p = payload(1, genesis_hash(), 0);
    let head = p.block_hash;
    api.new_payload(p).await;
    rx.try_recv().ok(); // consume newPayload event

    let fcs = ForkchoiceState {
        head_block_hash: head,
        safe_block_hash: genesis_hash(),
        finalized_block_hash: genesis_hash(),
    };
    let result = api.forkchoice_updated(fcs).await;
    assert_eq!(result.payload_status, PayloadStatus::Valid);
    assert_eq!(api.exec.head, head);
}

#[tokio::test]
async fn test_forkchoice_updated_unknown_head_syncing() {
    let (mut api, _rx) = engine().await;
    let fcs = ForkchoiceState {
        head_block_hash: [0xff; 32],
        safe_block_hash: genesis_hash(),
        finalized_block_hash: genesis_hash(),
    };
    let result = api.forkchoice_updated(fcs).await;
    assert_eq!(result.payload_status, PayloadStatus::Syncing);
}

#[tokio::test]
async fn test_forkchoice_reorg_detected() {
    let (mut api, mut rx) = engine().await;
    // fork A: blocks 1,2
    let p1 = payload(1, genesis_hash(), 0);
    let h1 = p1.block_hash;
    api.new_payload(p1).await;
    let p2a = payload(2, h1, 0);
    let h2a = p2a.block_hash;
    api.new_payload(p2a).await;

    // set head to block 2a
    api.forkchoice_updated(ForkchoiceState {
        head_block_hash: h2a,
        safe_block_hash: h1,
        finalized_block_hash: genesis_hash(),
    }).await;

    // drain events
    while rx.try_recv().is_ok() {}

    // fork B: different block 2 (different content → different hash)
    let mut p2b = payload(2, h1, 1000);
    p2b.block_hash[2] = 0xff; // make hash distinct
    let h2b = p2b.block_hash;
    api.exec.insert_block(engine_sync::BlockHeader {
        hash: h2b,
        parent_hash: h1,
        number: 2,
        gas_used: 1000,
    });

    // reorg to fork B
    api.forkchoice_updated(ForkchoiceState {
        head_block_hash: h2b,
        safe_block_hash: h1,
        finalized_block_hash: genesis_hash(),
    }).await;

    let events: Vec<EngineApiEvent> = {
        let mut v = vec![];
        while let Ok(e) = rx.try_recv() { v.push(e); }
        v
    };
    let has_reorg = events.iter().any(|e| matches!(e, EngineApiEvent::ChainReorg { .. }));
    assert!(has_reorg, "reorg event must be emitted on chain switch");
}

#[tokio::test]
async fn test_finalized_pointer_advances() {
    let (mut api, _rx) = engine().await;
    let p1 = payload(1, genesis_hash(), 0);
    let h1 = p1.block_hash;
    api.new_payload(p1).await;

    api.forkchoice_updated(ForkchoiceState {
        head_block_hash: h1,
        safe_block_hash: h1,
        finalized_block_hash: h1,
    }).await;

    assert_eq!(api.exec.finalized, h1);
}
