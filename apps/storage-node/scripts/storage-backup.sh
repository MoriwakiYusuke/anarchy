#!/bin/bash
# storage-backup.sh — hot backup of a running storage-node's redb DB.
#
# How it works:
#   1. Find the node's PID file (data/{node}/.pid or argv-supplied path)
#   2. SIGSTOP the process. redb finishes any in-flight syscall and is
#      paused. The on-disk file is in a write-consistent state because
#      redb commits via a single fsync per txn (see redb's MVCC notes).
#   3. cp -p the fragments.redb file to the destination, preserving mtime.
#   4. SIGCONT — service resumes.
#
# This is good enough for daily / weekly backups. The pause window is
# the time of one cp call (single file, kernel page cache hot).
#
# Limitations:
#   * Not a true point-in-time snapshot if you have multi-second writes
#     in flight when SIGSTOP fires — redb txns are atomic, so the file
#     is consistent, but a single in-flight commit may or may not appear.
#     For most operational use this is acceptable.
#   * Doesn't back up identity/ subdirectory by default (use --with-identity).
#
# Usage:
#   storage-backup.sh <node-name|all> <backup-dir>
#   storage-backup.sh node1 /var/backups/anarchy/
#   storage-backup.sh all   /var/backups/anarchy/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

usage() {
    cat <<EOF
Usage: $0 <node-name|all> <backup-dir> [--with-identity]

Examples:
  $0 node1 /var/backups/anarchy
  $0 all   /var/backups/anarchy --with-identity

Output: <backup-dir>/<node>-<timestamp>/fragments.redb
EOF
    exit 1
}

[[ $# -lt 2 ]] && usage

NODE="$1"
BACKUP_DIR="$2"
WITH_IDENTITY=0
[[ "${3:-}" == "--with-identity" ]] && WITH_IDENTITY=1

mkdir -p "$BACKUP_DIR"

backup_one() {
    local node="$1"
    local data_dir="$PROJECT_DIR/data/$node"
    local pid_file="$PROJECT_DIR/data/$node.pid"
    local redb_file="$data_dir/fragments.redb"

    if [[ ! -f "$redb_file" ]]; then
        echo -e "${YELLOW}skip $node: no $redb_file${NC}"
        return
    fi

    local timestamp
    timestamp="$(date -u +%Y%m%d-%H%M%SZ)"
    local out_dir="$BACKUP_DIR/$node-$timestamp"
    mkdir -p "$out_dir"

    local pid=""
    if [[ -f "$pid_file" ]]; then
        pid="$(cat "$pid_file")"
        if kill -0 "$pid" 2>/dev/null; then
            echo -e "${GREEN}SIGSTOP $node (PID $pid)${NC}"
            kill -STOP "$pid"
            # Always SIGCONT on exit, even if cp fails.
            trap "kill -CONT $pid 2>/dev/null || true" EXIT
        else
            echo -e "${YELLOW}$node PID file present but process gone — backing up offline${NC}"
            pid=""
        fi
    else
        echo -e "${YELLOW}$node not running — backing up offline${NC}"
    fi

    # `cp -p` preserves timestamps so subsequent rsync / dedupe works well.
    cp -p "$redb_file" "$out_dir/fragments.redb"
    echo -e "${GREEN}copied $redb_file → $out_dir/fragments.redb${NC}"

    if [[ "$WITH_IDENTITY" == 1 && -d "$data_dir/identity" ]]; then
        cp -rp "$data_dir/identity" "$out_dir/identity"
        echo -e "${GREEN}copied identity/${NC}"
    fi

    if [[ -n "$pid" ]]; then
        kill -CONT "$pid"
        trap - EXIT
        echo -e "${GREEN}SIGCONT $node${NC}"
    fi

    # Size report — useful for capacity planning.
    local size
    size="$(du -h "$out_dir/fragments.redb" | cut -f1)"
    echo -e "${GREEN}$node backup: $out_dir ($size)${NC}"
}

if [[ "$NODE" == "all" ]]; then
    shopt -s nullglob
    for d in "$PROJECT_DIR/data"/node*; do
        [[ -d "$d" ]] || continue
        backup_one "$(basename "$d")"
    done
else
    backup_one "$NODE"
fi

echo -e "${GREEN}done${NC}"
