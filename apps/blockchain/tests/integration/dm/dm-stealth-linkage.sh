#!/bin/bash
# feature 019-direct-messages T052 — DM stealth linkage 検証 (placeholder)
#
# 検証内容 (FR-003 / FR-024): Alice / Bob のメイン AccountId が以下のいずれにも
# 出現しないこと。
#   - DmDispatchesByBlock の dispatch.recipient_stealth
#   - DmDispatchesByBlock の dispatch.ephemeral_pubkey 由来 link
#   - storage-node に保存される fragment 内 (ciphertext は AES-GCM 済み)
#
# 必要な前準備 (T051 と共通):
#   - scripts/dm-publish-key.ts
#   - scripts/dm-send.ts
#   - jq + curl で JSON-RPC `state_getStorage` 系を直接叩く検査ロジック
#
# 現状未実装。Frontend / pallet 単体テスト (cargo test -p pallet-messaging
# 30 tests) で recipient_stealth ≠ counterparty AccountId は確認済み。

set -e

echo "[SKIP] dm-stealth-linkage: requires scripts/dm-* CLI helpers (T052)"
exit 0
