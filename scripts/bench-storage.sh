#!/bin/bash
# bench-storage.sh — drives `apps/storage-node/src/bin/bench_storage.rs`
# across the matrix specified in TODO §4.9 ({64KiB, 256KiB, 1MiB} × scale).
#
# Default scale = 10K fragments per cell (~few GiB of disk, runs in
# minutes on an SSD). Pass --full for the production-grade 1M cells, but
# expect to consume hundreds of GiB of disk and tens of minutes.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
BIN="$REPO_DIR/apps/storage-node/target/release/bench-storage"
TMPDIR="${TMPDIR:-/tmp}/anarchy-bench-storage"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

usage() {
    cat <<EOF
Usage: $0 [--quick | --full | --count N --sizes SIZES]

  --quick       (default) 10000 fragments × {64K, 256K, 1M}
  --full        1_000_000 fragments × {64K, 256K, 1M}  (TODO §4.9)
  --count N     custom fragment count per cell
  --sizes ...   comma-separated sizes in bytes (e.g. 65536,262144)

Output: TSV table (count, size, put_ops, get_ops, del_ops, scan_ops,
        put_p99_us, get_p99_us, startup_ms, on_disk_bytes) on stdout +
        a human-readable summary on stderr.
EOF
    exit 1
}

COUNT=10000
SIZES="65536,262144,1048576"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --quick) shift ;;
        --full)  COUNT=1000000; shift ;;
        --count) COUNT="$2"; shift 2 ;;
        --sizes) SIZES="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "unknown arg: $1" >&2; usage ;;
    esac
done

if [[ ! -x "$BIN" ]]; then
    echo -e "${YELLOW}Building bench-storage in release mode...${NC}" >&2
    (cd "$REPO_DIR/apps/storage-node" && cargo build --release --bin bench-storage)
fi

echo -e "${GREEN}=== bench-storage: count=$COUNT sizes=$SIZES ===${NC}" >&2
printf 'count\tsize\tput_ops\tget_ops\tdel_ops\tscan_ops\tput_p99_us\tget_p99_us\tstartup_ms\ton_disk\n'

IFS=',' read -ra size_array <<< "$SIZES"
for size in "${size_array[@]}"; do
    cell_dir="$TMPDIR-$COUNT-$size"
    echo -e "${YELLOW}>>> count=$COUNT size=$size dir=$cell_dir${NC}" >&2
    "$BIN" --count "$COUNT" --size "$size" --tmpdir "$cell_dir"
    rm -rf "$cell_dir"
done

echo -e "${GREEN}done${NC}" >&2
