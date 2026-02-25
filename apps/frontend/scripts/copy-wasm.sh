#!/bin/bash
# WASMファイルをpublic/wasmにコピーするスクリプト
# Web Worker内でWASMを正しくロードするために必要

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$(dirname "$SCRIPT_DIR")"
WASM_SOURCE="$FRONTEND_DIR/../../packages/wasm-engine/pkg/anarchy_wasm_engine_bg.wasm"
WASM_DEST_DIR="$FRONTEND_DIR/public/wasm"
WASM_DEST="$WASM_DEST_DIR/anarchy_wasm_engine_bg.wasm"

# WASMソースが存在するか確認
if [ ! -f "$WASM_SOURCE" ]; then
    echo "[copy-wasm] WASM source not found: $WASM_SOURCE"
    echo "[copy-wasm] Run 'cd packages/wasm-engine && wasm-pack build --target web --out-dir pkg' first"
    exit 0  # エラーではなく警告として終了
fi

# ディレクトリ作成
mkdir -p "$WASM_DEST_DIR"

# コピー
cp "$WASM_SOURCE" "$WASM_DEST"
echo "[copy-wasm] WASM copied to public/wasm/"
