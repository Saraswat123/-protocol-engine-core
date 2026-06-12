use engine_sync::{
    EngineApi, EngineApiEvent, ExecutionPayload, ForkchoiceState, PayloadAttributes,
    PayloadStatus,
};
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
    p.gas_limit = 15_000_000;
    let status = api.new_payload(p).await;
    assert!(matches!(status, PayloadStatus::Invalid { .. }));
}

#[tokio::test]
async fn test_chain_of_payloads() {
    let (mut api, _rx) = engine().await;
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

// ── forkchoiceUpdated (no attrs) ──────────────────────────────────────────────

#[tokio::test]
async fn test_forkchoice_updated_known_head() {
    let (mut api, mut rx) = engine().await;
    let p = payload(1, genesis_hash(), 0);
    let head = p.block_hash;
    api.new_payload(p).await;
    rx.try_recv().ok();

    let fcs = ForkchoiceState {
        head_block_hash: head,
        safe_block_hash: genesis_hash(),
        finalized_block_hash: genesis_hash(),
    };
    let result = api.forkchoice_updated(fcs, None).await;
    assert_eq!(result.payload_status, PayloadStatus::Valid);
    assert_eq!(result.payload_id, None);
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
    let result = api.forkchoice_updated(fcs, None).await;
    assert_eq!(result.payload_status, PayloadStatus::Syncing);
}

#[tokio::test]
async fn test_forkchoice_reorg_detected() {
    let (mut api, mut rx) = engine().await;
    let p1 = payload(1, genesis_hash(), 0);
    let h1 = p1.block_hash;
    api.new_payload(p1).await;
    let p2a = payload(2, h1, 0);
    let h2a = p2a.block_hash;
    api.new_payload(p2a).await;

    api.forkchoice_updated(ForkchoiceState {
        head_block_hash: h2a,
        safe_block_hash: h1,
        finalized_block_hash: genesis_hash(),
    }, None).await;

    while rx.try_recv().is_ok() {}

    let mut p2b = payload(2, h1, 1000);
    p2b.block_hash[2] = 0xff;
    let h2b = p2b.block_hash;
    api.exec.insert_block(engine_sync::BlockHeader {
        hash: h2b,
        parent_hash: h1,
        number: 2,
        gas_used: 1000,
    });

    api.forkchoice_updated(ForkchoiceState {
        head_block_hash: h2b,
        safe_block_hash: h1,
        finalized_block_hash: genesis_hash(),
    }, None).await;

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
    }, None).await;

    assert_eq!(api.exec.finalized, h1);
}

// ── PayloadAttributes / get_payload ──────────────────────────────────────────

fn attrs(timestamp: u64, txs: Vec<Vec<u8>>) -> PayloadAttributes {
    PayloadAttributes {
        timestamp,
        suggested_fee_recipient: [0xab; 20],
        prev_randao: [0u8; 32],
        transactions: txs,
    }
}

#[tokio::test]
async fn test_forkchoice_with_attrs_returns_payload_id() {
    let (mut api, _rx) = engine().await;
    let p = payload(1, genesis_hash(), 0);
    let head = p.block_hash;
    api.new_payload(p).await;

    let result = api.forkchoice_updated(ForkchoiceState {
        head_block_hash: head,
        safe_block_hash: genesis_hash(),
        finalized_block_hash: genesis_hash(),
    }, Some(attrs(24, vec![]))).await;

    assert_eq!(result.payload_status, PayloadStatus::Valid);
    assert!(result.payload_id.is_some(), "must return payload_id when attrs provided");
}

#[tokio::test]
async fn test_get_payload_returns_built_payload() {
    let (mut api, _rx) = engine().await;
    let p = payload(1, genesis_hash(), 0);
    let head = p.block_hash;
    api.new_payload(p).await;

    let result = api.forkchoice_updated(ForkchoiceState {
        head_block_hash: head,
        safe_block_hash: genesis_hash(),
        finalized_block_hash: genesis_hash(),
    }, Some(attrs(24, vec![vec![0x01, 0x02]]))).await;

    let id = result.payload_id.unwrap();
    let built = api.get_payload(id).expect("get_payload must return built payload");

    assert_eq!(built.parent_hash, head);
    assert_eq!(built.block_number, 2, "built block must extend head (block 1)");
    assert_eq!(built.timestamp, 24);
    assert_eq!(built.transactions.len(), 1, "forced tx must be included");
}

#[tokio::test]
async fn test_get_payload_consumes_job() {
    let (mut api, _rx) = engine().await;
    let p = payload(1, genesis_hash(), 0);
    let head = p.block_hash;
    api.new_payload(p).await;

    let result = api.forkchoice_updated(ForkchoiceState {
        head_block_hash: head,
        safe_block_hash: genesis_hash(),
        finalized_block_hash: genesis_hash(),
    }, Some(attrs(36, vec![]))).await;

    let id = result.payload_id.unwrap();
    assert!(api.get_payload(id).is_some());
    assert!(api.get_payload(id).is_none(), "second call must return None (consumed)");
}

#[tokio::test]
async fn test_full_round_trip_fcu_newpayload() {
    let (mut api, _rx) = engine().await;

    // block 1 exists as parent
    let p1 = payload(1, genesis_hash(), 0);
    let h1 = p1.block_hash;
    api.new_payload(p1).await;

    // CL calls forkchoiceUpdated with attrs → gets payload_id
    let result = api.forkchoice_updated(ForkchoiceState {
        head_block_hash: h1,
        safe_block_hash: h1,
        finalized_block_hash: genesis_hash(),
    }, Some(attrs(24, vec![vec![0xde, 0xad]]))).await;

    let id = result.payload_id.unwrap();

    // CL retrieves built payload
    let built = api.get_payload(id).unwrap();

    // CL submits it back via newPayload
    let status = api.new_payload(built).await;
    assert_eq!(status, PayloadStatus::Valid, "self-built payload must be valid");
}

#[tokio::test]
async fn test_multiple_payload_ids_independent() {
    let (mut api, _rx) = engine().await;
    let p = payload(1, genesis_hash(), 0);
    let head = p.block_hash;
    api.new_payload(p).await;

    let fcs = ForkchoiceState {
        head_block_hash: head,
        safe_block_hash: genesis_hash(),
        finalized_block_hash: genesis_hash(),
    };

    let r1 = api.forkchoice_updated(fcs.clone(), Some(attrs(12, vec![]))).await;
    let r2 = api.forkchoice_updated(fcs, Some(attrs(24, vec![]))).await;

    let id1 = r1.payload_id.unwrap();
    let id2 = r2.payload_id.unwrap();
    assert_ne!(id1, id2, "each forkchoice+attrs call must get unique payload_id");

    assert!(api.get_payload(id1).is_some());
    assert!(api.get_payload(id2).is_some());
}
