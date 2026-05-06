import { test, expect } from './fixtures/chain';

/**
 * Thread reply E2E (feature/thread-reply ブランチ用)。
 *
 * 検証範囲 — フロントが追加した X 風スレッド返信 UX が end-to-end で機能することを確認:
 *   - 親投稿をトップレベルの PostForm で作成 → タイムラインに出る
 *   - 親 PostItem の Reply ボタンでインライン PostForm が展開する
 *   - インライン PostForm で `parent_id` 付き create_post が finalize する
 *   - 送信成功で返信フォームが自動で閉じる
 *   - 親に "View 1 reply" バッジ (replyCount > 0) が表示される
 *   - バッジクリックで返信本文がネスト表示され、"Reply to #N" バッジが付く
 *
 * Goal: チェーン側 parent_id 配線とフロントのネスト表示・インライン送信の連結が
 *       本物のブラウザで通ることを示す。
 */
test.describe('Thread reply flow', () => {
  test('Alice posts and replies to herself with X-style nested display', async ({
    page,
    connectDevAccount,
  }) => {
    await connectDevAccount('Alice');

    const tag = `e2e-thread-reply-${Date.now()}`;
    const parentBody = `${tag}-parent`;
    const replyBody = `${tag}-child`;

    // ---- 1. 親投稿: トップレベル PostForm から送信 ----
    // ページ最上部の PostForm が最初の <form>。
    const topForm = page.locator('form').first();
    await expect(topForm.getByPlaceholder("What's happening?")).toBeVisible({ timeout: 30_000 });
    await topForm.getByPlaceholder("What's happening?").fill(parentBody);

    const topSubmit = topForm.getByRole('button', { name: /^Post$/, exact: true });
    await expect(topSubmit).toBeEnabled({ timeout: 30_000 });
    await topSubmit.click();

    // create_post finalize → "Posted! (Block #N)" success メッセージ
    await expect(page.getByText(/Posted!\s*\(Block #\d+\)/).first()).toBeVisible({
      timeout: 120_000,
    });

    // ---- 2. 親投稿がタイムラインに現れるのを待つ ----
    // PostItem は本文を Storage Node から復元してから描画するので最大 60s 許容。
    const parentArticle = page.locator('article').filter({ hasText: parentBody });
    await expect(parentArticle).toBeVisible({ timeout: 60_000 });

    // ---- 3. 親 PostItem の Reply ボタンを押す ----
    const parentReplyBtn = parentArticle.getByRole('button', { name: /^Reply$/ });
    await expect(parentReplyBtn).toBeVisible({ timeout: 10_000 });
    await parentReplyBtn.click();

    // ---- 4. インライン返信フォームに本文を入れて送信 ----
    // インラインフォームは "Replying to #N" ヘッダーを持つ <form>。
    const replyForm = page.locator('form').filter({ hasText: /Replying to #\d+/ });
    await expect(replyForm).toBeVisible({ timeout: 10_000 });

    await replyForm.getByPlaceholder("What's happening?").fill(replyBody);
    const replySubmit = replyForm.getByRole('button', { name: /^Post$/, exact: true });
    await expect(replySubmit).toBeEnabled({ timeout: 30_000 });
    await replySubmit.click();

    // ---- 5. 送信成功で返信フォームが自動的に閉じる ----
    // onPostSuccess → setReplyFormOpen(false) で <form> 自体が消える。
    // create_post finalize 待ちで最大 120s。
    await expect(replyForm).toBeHidden({ timeout: 120_000 });

    // ---- 6. 送信直後は repliesExpanded=true で自動展開され "Hide replies" バッジが見える ----
    // Timeline の refreshTrigger が onReplyPosted で bump されて再 fetch → replyCount=1 で出現。
    // 前回 run で残った post も含み複数候補が出るので親 article 内にスコープする。
    const parentRoot = parentArticle.locator('xpath=..');
    const hideRepliesBtn = parentRoot.getByRole('button', { name: /Hide replies/ });
    await expect(hideRepliesBtn).toBeVisible({ timeout: 60_000 });

    // ---- 7. 返信本文がネスト表示されている ----
    // Storage Node からの復元込みで最大 60s。
    await expect(parentRoot.getByText(replyBody, { exact: false })).toBeVisible({
      timeout: 60_000,
    });

    // ---- 8. 返信カードに "Reply to #N" バッジが付いている ----
    const replyArticle = parentRoot.locator('article').filter({ hasText: replyBody });
    await expect(replyArticle.getByText(/Reply to #\d+/)).toBeVisible({ timeout: 10_000 });

    // ---- 9. "Hide replies" 押下で折りたたまれ、"View 1 reply" に切り替わる ----
    await hideRepliesBtn.click();
    await expect(parentRoot.getByRole('button', { name: /View 1 reply/ })).toBeVisible({
      timeout: 10_000,
    });
  });
});
