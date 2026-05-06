/**
 * Thread reply のグルーピング用ヘルパー。
 *
 * チェーンは Post.parent_id (Option<u64>) しか持たないため、
 * 「ある post が属するスレッドの root」と「root ごとに紐づく返信」の集計は
 * フロント側で全 post を見て計算する必要がある。
 *
 * 設計: docs/superpowers/specs/2026-05-06-thread-reply-frontend-design.md
 */

export interface ThreadablePost {
  id: number
  parentId: number | null
  createdAt: number
}

/**
 * 各 post id に対して、所属スレッドの root id を返す Map を計算する。
 *
 * - `parentId === null` → 自身が root
 * - 親が `posts` セットに存在しない (削除済み等) → orphan として **自身を root** として扱う
 * - 循環参照 (バグで親 = 自分や a→b→a) → 訪問済みノードに到達した時点で打ち切り、
 *   そこを root とする (無限ループ回避)
 *
 * 計算量: O(N + chain depth) — 各ノードを高々 1 回辿り、結果をキャッシュする。
 */
export function computeRootMap<P extends ThreadablePost>(
  posts: ReadonlyArray<P>,
): Map<number, number> {
  const index = new Map<number, P>()
  for (const p of posts) index.set(p.id, p)

  const rootCache = new Map<number, number>()

  for (const start of posts) {
    if (rootCache.has(start.id)) continue

    const path: number[] = []
    const visited = new Set<number>()
    let cur: number = start.id

    while (true) {
      if (rootCache.has(cur)) {
        // すでに解決済みの祖先を見つけたら、そのキャッシュ値を path 全体に伝播させる
        const root = rootCache.get(cur)!
        for (const id of path) rootCache.set(id, root)
        break
      }
      if (visited.has(cur)) {
        // 循環: cur 自身を root として扱い、path 全体に伝播
        for (const id of path) rootCache.set(id, cur)
        rootCache.set(cur, cur)
        break
      }
      visited.add(cur)
      path.push(cur)

      const node = index.get(cur)
      if (!node || node.parentId === null) {
        // 親が無い (root 自身) → cur が root
        for (const id of path) rootCache.set(id, cur)
        break
      }
      if (!index.has(node.parentId)) {
        // orphan: 親 id が posts に存在しない → cur 自身を root として扱う
        // (親 id を root にしてしまうと posts に無いノードが root として残るため誤り)
        for (const id of path) rootCache.set(id, cur)
        break
      }
      cur = node.parentId
    }
  }

  return rootCache
}

/**
 * root id ごとに返信 (root 自身を除く) を集計し、createdAt 昇順 (古い→新しい) で並べる。
 */
export function groupRepliesByRoot<P extends ThreadablePost>(
  posts: ReadonlyArray<P>,
  rootMap: ReadonlyMap<number, number>,
): Map<number, P[]> {
  const repliesByRoot = new Map<number, P[]>()
  for (const p of posts) {
    const root = rootMap.get(p.id)
    if (root === undefined || root === p.id) continue
    const arr = repliesByRoot.get(root) ?? []
    arr.push(p)
    repliesByRoot.set(root, arr)
  }
  for (const arr of repliesByRoot.values()) {
    arr.sort((a, b) => a.createdAt - b.createdAt)
  }
  return repliesByRoot
}

/**
 * トップレベル候補かどうか: parent が無いか、orphan の場合に true。
 * 内部的には rootMap で `id === rootMap.get(id)` と等価だが、独立に呼べる便利関数。
 */
export function isTopLevel<P extends ThreadablePost>(
  post: P,
  postIndex: ReadonlyMap<number, P>,
): boolean {
  return post.parentId === null || !postIndex.has(post.parentId)
}
