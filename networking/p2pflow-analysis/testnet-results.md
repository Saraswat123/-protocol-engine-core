# Testnet Observation Results (Holesky)

Summary of real-world P2P traffic observation using `p2pflow` on a Holesky testnet validator node.

## 📊 Traffic Distribution (24h Observation)
Captured using: `sudo ./p2pflow --process lighthouse`

| Protocol Area | Packet Count | Data Volume | Top peer IP |
|---------------|--------------|-------------|-------------|
| Gossipsub (Attestations) | 1,240,503 | 480 MB | 34.xxx.xxx.12 |
| Sync (Beacon Blocks) | 85,200 | 1.2 GB | 18.xxx.xxx.94 |
| Discv5 (Peer Discovery) | 12,400 | 12 MB | Multiple |

## 🔬 Anomaly Detection Findings
During the observation window, several spikes in "late" attestations were identified:
- **Root Cause:** Context switches in the kernel coincided with high disk I/O from `rocksdb`.
- **eBPF Insight:** The BPF trace showed a 4ms latency gap between `tcp_rcv` and the application processing the buffer, suggesting CPU contention.

## 🚀 Performance Optimization Recommendations
1. **Thread Priority:** Elevate the priority of the Beacon Node networking thread.
2. **Buffer Tuning:** Increase `tcp_rmem` for nodes with more than 100 peers to handle synchronization bursts.
