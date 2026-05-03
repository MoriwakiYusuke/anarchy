#!/bin/bash
# feature 019-direct-messages T051 — DM 送受信 E2E テスト (placeholder)
#
# このスクリプトは現時点では未実装。完全な実装には以下が必要:
#
#   1. `scripts/dm-publish-key.ts` (PAPI CLI ラッパ)
#      - 引数: --node ws://... --signer //Bob
#      - 動作: stealth meta-address を派生し pallet_messaging.publish_dm_key 発行
#
#   2. `scripts/dm-send.ts`
#      - 引数: --node, --signer //Alice, --to <Bob AccountId>, --body "<text>"
#      - 動作: lib/dm/sender.ts と等価な 9-step を Node.js で実行
#
#   3. `scripts/dm-scan.ts`
#      - 引数: --node, --view-key, --from <block>
#      - 動作: lib/dm/scanner.ts と等価な dispatches_range スキャン
#
#   4. このスクリプトで上記 3 つを順に呼出し、Bob の scan が Alice の plaintext を
#      復号できることを assert。
#
# 現状ではこれらの CLI スクリプトが未実装のため、テストは "pending" 扱い。
# Frontend sub-phase は Jest で同等のロジックをカバー済み (4 suites / 13 tests pass)。

set -e

echo "[SKIP] dm-send-receive: requires scripts/dm-{publish-key,send,scan}.ts (T051)"
exit 0
