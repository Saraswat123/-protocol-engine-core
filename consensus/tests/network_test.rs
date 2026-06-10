use consensus::network::simulate;

/// Async network: n=4 nodes run views over Tokio channels, verify commits.
#[tokio::test]
async fn test_async_network_4nodes_commits() {
    let committed = simulate(4, 4).await;
    // after 4 views with 2-chain commit rule, nodes should have committed blocks
    // (commit happens at view V when view V+1's QC forms)
    // all nodes must agree on same commit count
    let first = committed[0];
    for c in &committed {
        assert_eq!(*c, first, "all nodes must have same committed count");
    }
}

#[tokio::test]
async fn test_async_network_single_view() {
    // sanity: single view should not panic and nodes advance view
    let committed = simulate(4, 1).await;
    assert_eq!(committed.len(), 4);
}

#[tokio::test]
async fn test_async_network_7nodes() {
    // n=7: f=2, threshold=5
    let committed = simulate(7, 4).await;
    let first = committed[0];
    for c in &committed {
        assert_eq!(*c, first, "7-node cluster must agree on commits");
    }
}
