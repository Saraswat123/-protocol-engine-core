# eBPF Architecture in p2pflow

Analysis of how `p2pflow` hooks into the Linux kernel to observe Ethereum execution and consensus client communication.

## 🏗️ BPF Program Structure
- **Kprobes & Tracepoints:** The program attaches to `tcp_sendmsg` and `tcp_cleanup_rbuf` in the kernel to capture packets before they are encrypted or after they are decrypted by the application layer.
- **Maps:** Uses `BPF_MAP_TYPE_HASH` to track connection state (IP, Port, Process ID) and `BPF_MAP_TYPE_RINGBUF` for high-speed data transfer to userspace.

## 📡 Observation Flow
1. **Filter by PID:** The userspace agent finds the PID of the Ethereum process (e.g., `geth` or `lighthouse`).
2. **Global Tracking:** The BPF program only captures packets associated with that specific process ID.
3. **SSZ/RLP Decoding:** Userspace reads the raw bytes from the BPF ring buffer and decodes Ethereum-specific protocols.

## 🔬 Protocol Dissection
- **Execution Layer:** Monitors RLP-encoded engine API calls and devp2p gossip.
- **Consensus Layer:** Observes SSZ-encoded Beacon Node traffic, specifically targeting Gossipsub noise and validation messages.
