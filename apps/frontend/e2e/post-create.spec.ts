import { test, expect } from './fixtures/chain';

/**
 * Post 作成 E2E (chore/maintenance-sweep ブランチの回帰防止用)。
 *
 * 検証範囲 — このブランチで触った下記の path が壊れていないことを一気通貫で見る:
 *   - Frontend useUpload (#32-CRITICAL-2 CHAIN_NODE_RPC_ENDPOINTS 命名整理)
 *   - Chain node `storage_uploadFragment` (#28-CRIT-3 register_endpoint rate limit
 *     等を加えても通常 upload は影響を受けない)
 *   - Storage node の atomic quota (#31-H-1) と CORS allowlist (#30-C-1) と
 *     URL validator (#31-H-5) — 既存 happy-path で reject されないこと
 *   - libp2p Put auth (#30-C-2) — 通常 upload は HTTP RPC ルートで libp2p Put は
 *     通らないので reject されないこと
 *   - Post pallet create_post — B カテゴリで pallet ロジックを触ったが
 *     create_post パスは変えていないので動くこと
 *   - Wasm (KZG split / commitment / proof / hybrid encrypt) — vss.rs unwrap 排除
 *     や regenerate_share commit-verify (#34-FINDING-02) で encrypt path を壊して
 *     いないこと
 *
 * Goal: "Posted! (Block #N)" が success status に出れば全部通った証拠。
 */
test.describe('Post creation flow', () => {
  test('Alice posts text content end-to-end', async ({ page, connectDevAccount }) => {
    await connectDevAccount('Alice');

    // PostForm の textarea (placeholder = "What's happening?") に書き込む
    const textarea = page.getByPlaceholder("What's happening?");
    await expect(textarea).toBeVisible({ timeout: 30_000 });

    const body = `e2e maintenance-sweep ${Date.now()}`;
    await textarea.fill(body);

    // Submit ボタン (post.submit = "Post")。usePostCost の Loading 完了を待ってから押す。
    const submitBtn = page.getByRole('button', { name: /^Post$/, exact: true });
    await expect(submitBtn).toBeEnabled({ timeout: 30_000 });
    await submitBtn.click();

    // create_post finalize → "Posted! (Block #N)" success メッセージが出る。
    // upload (KZG split + storage upload) + extrinsic finalize で 60s 超は普通。
    await expect(page.getByText(/Posted!\s*\(Block #\d+\)/)).toBeVisible({ timeout: 120_000 });
  });
});
