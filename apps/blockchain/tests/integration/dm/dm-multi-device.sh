#!/bin/bash
# feature 019-direct-messages: 複数デバイス間のバックアップ復元統合テスト (スタブ)
#
# Phase 4 (T094 関連) で以下のシナリオを実装する:
#   1. デバイス A で DM 受信 + block list + read-receipt opt-out (FR-022, FR-016b)
#   2. 暗号化バックアップ (AES-GCM + PBKDF2 100k) をエクスポート
#   3. デバイス B で import し状態が一致することを確認
#
# 現状は placeholder (他の DM スクリプトと揃えて SKIP 扱い)。
# CI / `pnpm test:dm` が常に失敗するのを避けるため `exit 0` で終了する。

set -e

echo "[SKIP] dm-multi-device: pending — requires backup export/import CLI (Phase 4)"
exit 0
