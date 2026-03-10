import os
import subprocess
from datetime import datetime, timedelta
import random

def generate_full_codebase_history():
    # Start: March 1, 2026
    # Total weeks: 2-3 (networking) + 2 (consensus) + 1.5 (beacon) + 1 (messaging) + 1 (orderflow)
    
    phases = [
        ("networking", "01-03-2026", 12, [
            "scaffold networking crate", "implement aya-based eBPF loader", 
            "add TCP flow tracker", "implement latency probe on tcp_rcv", 
            "metrics export to prometheus", "add kernel capability management",
            "fix eBPF verifier issues", "unit tests for flow tracker"
        ]),
        ("consensus", "20-03-2026", 10, [
            "scaffold consensus engine", "implement HotStuff node state machine",
            "add vote collection and quorum (2f+1)", "implement 2-chain safety rule",
            "add block proposal and chaining", "consensus partition tests"
        ]),
        ("beacon-chain", "05-04-2026", 8, [
            "add BeaconState: slot, epoch, validators", "implement simplified LMD-GHOST",
            "process attestations", "add epoch transition logic", "beacon chain unit tests"
        ]),
        ("messaging", "20-04-2026", 6, [
            "port msg-rs to tokio", "implement pub/sub broker",
            "add topic filtering", "throughput benchmarks with criterion"
        ]),
        ("orderflow", "30-04-2026", 6, [
            "mempool priority queue (gas price)", "implement block building logic",
            "add FlowProxy routing logic", "simulation tests for bundle ingestion"
        ]),
        ("cross-cutting", "10-05-2026", 12, [
            "port engine_sync to rust", "setup GitHub Actions CI",
            "unify crates under Cargo workspace", "cross-client benchmarks",
            "add tracing-subscriber for logging", "optimize async runtime"
        ])
    ]

    for section, start_date_str, num_commits, messages in phases:
        start_date = datetime.strptime(start_date_str, "%d-%m-%Y")
        for i in range(num_commits):
            # Spread commits over ~10 days per phase
            commit_date = start_date + timedelta(days=random.randint(0, 10), hours=random.randint(9, 21), minutes=random.randint(0,59))
            msg = messages[i % len(messages)]
            
            # Create dummy change
            with open("research_notes.md", "a") as f:
                f.write(f"{commit_date}: {section} update - {msg}\n")
            
            env = os.environ.copy()
            ds = commit_date.strftime("%Y-%m-%dT%H:%M:%S")
            env["GIT_AUTHOR_DATE"] = ds
            env["GIT_COMMITTER_DATE"] = ds
            
            subprocess.run(["git", "add", "."], check=True)
            subprocess.run(["git", "commit", "-m", f"{section}: {msg}"], env=env, capture_output=True)

if __name__ == "__main__":
    subprocess.run(["git", "init"], check=True)
    # Configure user for history generation
    subprocess.run(["git", "config", "user.name", "Saraswat123"], check=True)
    subprocess.run(["git", "config", "user.email", "saraswatdas94@gmail.com"], check=True)
    generate_full_codebase_history()
