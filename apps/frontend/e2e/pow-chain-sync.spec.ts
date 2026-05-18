import { test, expect } from './fixtures/chain';

/**
 * PoW chain sync 回帰確認 E2E (Phase B)。
 *
 * Phase B 移行で frontend は smoldot を撤去して WebSocket (`getWsProvider`) に統一。
 * 元々 storage 系 RPC (`storage_uploadFragment` 等) は chain-node に直接 HTTP RPC を
 * 投げる構造で、smoldot はチェーン読み取りと標準 extrinsic 専用 — しかし PoW chain の
 * block digest を smoldot が verify できないため (BadBlockAnnounce(DecodeBlockAnnounceError))
 * 役立たずになっていた。WS に統一することで:
 *   - PoW 互換性問題が解消
 *   - bundle size が削減 (smoldot wasm ~数 MB)
 *   - storage 系で既に存在する WS 接続経路と統合 (architectural clarity)
 *
 * Anonymity 原則 (CLAUDE.md Principle #1) は chain-node を Tor hidden service として
 * 公開して `wss://<onion>:9944` で接続する運用で担保 (docs/operations/tor-overview.md)。
 *
 * 前提: dev node または testnet が起動済み (\`pnpm dev:node\` or \`pnpm testnet:start\`)
 *       環境変数 NEXT_PUBLIC_CHAIN_RPC_URL で endpoint を上書き可、デフォルトは
 *       ws://127.0.0.1:9944
 */

test.describe('PoW chain sync (WS provider)', () => {
  test('chain client connects and stays connected on PoW chain', async ({ page }) => {
    await page.goto('/');

    // 1. WS 接続成功 (Aura → PoW 切替後も chain RPC が応答すること)
    await expect(page.getByText('Connected', { exact: false })).toBeVisible({ timeout: 60_000 });

    // 2. 接続が一過性でないことを確認: error / 切断状態に遷移しないことを期待。
    //    waitForTimeout は anti-pattern なので、useChain の updateInterval (10s) を
    //    複数サイクル待つ間 "Connected" が常時 visible / "Connecting..."/エラーが
    //    visible にならない、という条件で expect.toPass を使う。
    await expect(async () => {
      await expect(page.getByText('Connected', { exact: false })).toBeVisible();
      await expect(
        page.getByText(/Connecting\.\.\.|エラー|タイムアウト/),
      ).not.toBeVisible();
    }).toPass({ timeout: 45_000, intervals: [5_000, 5_000, 10_000] });
  });

  test('Wallet connect → PoW chain で extrinsic 関連の RPC が叩ける', async ({
    page,
    connectDevAccount,
  }) => {
    await connectDevAccount('Alice');

    // PostForm が表示される = Wallet 経由で chain RPC (system_account / runtime API) が
    // 全部応答している証拠。Aura 関連 RuntimeApi を runtime から削除した後も壊れていないか。
    const textarea = page.getByPlaceholder("What's happening?");
    await expect(textarea).toBeVisible({ timeout: 30_000 });

    // 残高表示 (BalanceDisplay) もマウントされていること = system_account が叩けた
    // (PoW migration で frame_system::Config は変更していないが、新 pallet 統合の
    //  副作用が無いかを確認)
    const walletPanel = page.locator('aside');
    await expect(walletPanel.getByText(/MORAL/i).first()).toBeVisible({ timeout: 30_000 });
  });
});
