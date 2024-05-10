# Environment Setup: eBPF & Kernel Observation

Instructions and technical requirements for deploying `p2pflow` for low-level Ethereum P2P traffic analysis.

## 🛠️ System Dependencies
To compile and load BPF programs, the following toolchain is required:
```bash
sudo apt-get install pkg-config clang llvm libelf-dev libpcap-dev \
                     gcc-multilib build-essential \
                     linux-tools-$(uname -r)
```

## 🧠 Kernel Requirements (CO-RE & BTF)
The project utilizes **Compile Once – Run Everywhere (CO-RE)** technology.
- **Kernel Version:** 5.0+ (Ubuntu 21.04+ recommended).
- **BTF Verification:**
  ```bash
  cat /boot/config-$(uname -r) | grep CONFIG_DEBUG_INFO_BTF
  ```

## 🏗️ Generating `vmlinux.h`
To allow the BPF program to read kernel structures, we dump the BPF Type Format (BTF) definitions from the running kernel:
```bash
bpftool btf dump file /sys/kernel/btf/vmlinux format c > src/bpf/vmlinux.h
```

## 🚀 Build & Installation
```bash
# Clone with submodules for bundled libbpf
git clone --recurse-submodules https://github.com/netbound/p2pflow
cd p2pflow

# Build release binary
cargo build --release

# Optional: Install with capabilities (avoids sudo requirement)
make install
```
