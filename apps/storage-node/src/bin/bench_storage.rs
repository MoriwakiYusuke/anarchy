//! `bench_storage` — micro-benchmark for `FragmentStore` (TODO §4.9).
//!
//! Runs put / get / delete / range_scan against a fresh redb-backed
//! FragmentStore at the requested scale and prints a single-line TSV
//! result for easy aggregation.
//!
//! Note on numbers: this is a single-thread, no-network bench — meaningful
//! for "is the storage layer itself a bottleneck?" but not a substitute
//! for end-to-end load tests with libp2p / chain RPC in the loop.
//!
//! Usage:
//!   bench-storage --count 10000 --size 65536 --tmpdir /tmp/bench
//!
//! Output (TSV, one line):
//!   count  size  put_ops_per_s  get_ops_per_s  del_ops_per_s  scan_ops_per_s  put_p99_us  get_p99_us  startup_ms  on_disk_bytes
//!
//! For 1M-fragment runs, prefer SSDs with at least 2× (count × size)
//! free disk so the post-write compaction has headroom.

use std::path::Path;
use std::time::Instant;

use anarchy_storage_node::storage::FragmentStore;
use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "bench-storage")]
struct Args {
    /// Number of fragments to insert.
    #[arg(long, default_value_t = 10_000)]
    count: usize,
    /// Fragment size in bytes.
    #[arg(long, default_value_t = 65_536)]
    size: usize,
    /// Where to put the redb file. Will be wiped first.
    #[arg(long, default_value = "/tmp/anarchy-bench-storage")]
    tmpdir: String,
    /// Capacity (bytes). Default = count * size * 2.
    #[arg(long)]
    capacity: Option<u64>,
    /// Skip the delete phase (cheaper if you only care about put/get).
    #[arg(long, default_value_t = false)]
    skip_delete: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let dir = Path::new(&args.tmpdir);
    if dir.exists() {
        std::fs::remove_dir_all(dir).context("clear tmpdir")?;
    }
    std::fs::create_dir_all(dir).context("create tmpdir")?;

    let capacity = args
        .capacity
        .unwrap_or((args.count as u64) * (args.size as u64) * 2);
    let payload = vec![0xAB_u8; args.size];

    // ---- Open ----
    let open_t = Instant::now();
    let store = FragmentStore::new(dir.to_str().unwrap(), capacity)?;
    let startup_ms = open_t.elapsed().as_millis() as u64;

    // ---- PUT ----
    let mut put_latencies = Vec::with_capacity(args.count);
    let put_t = Instant::now();
    for i in 0..args.count {
        let id = make_id(i);
        let t = Instant::now();
        store.store(id, &payload)?;
        put_latencies.push(t.elapsed().as_micros() as u64);
    }
    let put_total = put_t.elapsed().as_secs_f64();
    let put_ops = (args.count as f64) / put_total;
    let put_p99 = pctl(&mut put_latencies, 0.99);

    // ---- GET ----
    let mut get_latencies = Vec::with_capacity(args.count);
    let get_t = Instant::now();
    for i in 0..args.count {
        let id = make_id(i);
        let t = Instant::now();
        let _ = store.retrieve(&id)?.expect("must exist");
        get_latencies.push(t.elapsed().as_micros() as u64);
    }
    let get_total = get_t.elapsed().as_secs_f64();
    let get_ops = (args.count as f64) / get_total;
    let get_p99 = pctl(&mut get_latencies, 0.99);

    // ---- RANGE SCAN ----
    // Use the post_fragments table because that's where range matters in
    // production. Capped at MAX_FRAGMENT_INDEX (255) — that's the legitimate
    // upper bound for shards-per-post in the protocol, so scanning that
    // many is a realistic worst case.
    let scan_count = args.count.min(256);
    for i in 0..scan_count as u32 {
        store.store_post_fragment(0xBEEF, i, &payload)?;
    }
    let scan_t = Instant::now();
    let _ = store.list_post_fragments(0xBEEF)?;
    let scan_total = scan_t.elapsed().as_secs_f64();
    // Express as items-scanned/s so it's comparable across runs.
    let scan_ops = scan_count as f64 / scan_total.max(1e-9);

    // ---- DELETE ----
    let del_ops = if args.skip_delete {
        0.0
    } else {
        let del_t = Instant::now();
        for i in 0..args.count {
            let id = make_id(i);
            let _ = store.delete(&id)?;
        }
        let del_total = del_t.elapsed().as_secs_f64();
        (args.count as f64) / del_total
    };

    let on_disk = file_size(dir.join("fragments.redb")).unwrap_or(0);

    // Single TSV line — easy for the shell wrapper to aggregate.
    println!(
        "{}\t{}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\t{}\t{}\t{}\t{}",
        args.count,
        args.size,
        put_ops,
        get_ops,
        del_ops,
        scan_ops,
        put_p99,
        get_p99,
        startup_ms,
        on_disk,
    );

    Ok(())
}

/// Deterministic 32-byte ID from a counter.
fn make_id(i: usize) -> [u8; 32] {
    let mut id = [0u8; 32];
    id[..8].copy_from_slice(&(i as u64).to_be_bytes());
    id
}

fn pctl(values: &mut [u64], q: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let idx = ((values.len() as f64) * q).min((values.len() - 1) as f64) as usize;
    values[idx]
}

fn file_size(p: impl AsRef<Path>) -> Option<u64> {
    std::fs::metadata(p).ok().map(|m| m.len())
}
