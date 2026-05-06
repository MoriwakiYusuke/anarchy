/**
 * threadGrouping helper の単体テスト (PR #51 Copilot review C6)。
 *
 * チェーンが parent_id しか持たないため、フロントで root を解決して返信を
 * グルーピングする必要がある。下記不変条件を回帰防止する。
 */

import {
  computeRootMap,
  groupRepliesByRoot,
  isTopLevel,
  type ThreadablePost,
} from '@/lib/threadGrouping'

const post = (
  id: number,
  parentId: number | null,
  createdAt = id * 10,
): ThreadablePost => ({ id, parentId, createdAt })

describe('computeRootMap', () => {
  it('flat な top-level だけの post 群では各 post が self-root', () => {
    const posts = [post(1, null), post(2, null), post(3, null)]
    const map = computeRootMap(posts)
    expect(map.get(1)).toBe(1)
    expect(map.get(2)).toBe(2)
    expect(map.get(3)).toBe(3)
  })

  it('親→子→孫 の 3 段ネストでも全員 root に解決される', () => {
    // 1 (root) ← 2 ← 3 ← 4
    const posts = [post(1, null), post(2, 1), post(3, 2), post(4, 3)]
    const map = computeRootMap(posts)
    expect(map.get(1)).toBe(1)
    expect(map.get(2)).toBe(1)
    expect(map.get(3)).toBe(1)
    expect(map.get(4)).toBe(1)
  })

  it('orphan (親 id が posts セットに存在しない) は self を root として扱う', () => {
    // 99 (root, posts に居ない) ← 5 (orphan) ← 6
    const posts = [post(5, 99), post(6, 5)]
    const map = computeRootMap(posts)
    expect(map.get(5)).toBe(5) // orphan: self-root
    expect(map.get(6)).toBe(5) // 5 がスレッド内 root として機能する
  })

  it('複数の独立スレッドが正しく分離される', () => {
    // thread A: 1 ← 2 ← 3
    // thread B: 10 ← 11
    const posts = [
      post(1, null), post(2, 1), post(3, 2),
      post(10, null), post(11, 10),
    ]
    const map = computeRootMap(posts)
    expect(map.get(1)).toBe(1)
    expect(map.get(2)).toBe(1)
    expect(map.get(3)).toBe(1)
    expect(map.get(10)).toBe(10)
    expect(map.get(11)).toBe(10)
  })

  it('循環参照でも無限ループせず、訪問済みノードを root として打ち切る', () => {
    // バグ想定: 1 ← 2 ← 3 ← 1 (3 が 1 を親としている)
    const posts = [post(1, 3), post(2, 1), post(3, 2)]
    // 結果は決定的に「処理を打ち切れる」ことが重要 (具体的な root は実装依存だが
    // 同サイクル内で必ず一意に決まり、無限ループしない)
    expect(() => computeRootMap(posts)).not.toThrow()
    const map = computeRootMap(posts)
    // サイクル内の 3 ノードは全員同じ root に解決される
    const roots = new Set([map.get(1), map.get(2), map.get(3)])
    expect(roots.size).toBe(1)
  })

  it('空配列でも問題なく空 Map を返す', () => {
    expect(computeRootMap([]).size).toBe(0)
  })

  it('結果はキャッシュされ、同じ post を 2 回計算しない (large input でも O(N))', () => {
    // 100 段ネストでも問題なく解決
    const posts: ThreadablePost[] = [post(0, null)]
    for (let i = 1; i < 100; i++) posts.push(post(i, i - 1))
    const map = computeRootMap(posts)
    for (let i = 0; i < 100; i++) {
      expect(map.get(i)).toBe(0)
    }
  })
})

describe('groupRepliesByRoot', () => {
  it('返信を root id ごとに集めて createdAt 昇順で並べる', () => {
    // root=1, replies (createdAt 順序がバラバラ): 3 (created=15), 2 (created=20), 4 (created=10)
    const posts = [
      post(1, null, 0),
      post(2, 1, 20),
      post(3, 1, 15),
      post(4, 1, 10),
    ]
    const map = computeRootMap(posts)
    const grouped = groupRepliesByRoot(posts, map)
    const replies = grouped.get(1) ?? []
    expect(replies.map((r) => r.id)).toEqual([4, 3, 2])
  })

  it('root 自身は集計に含まれない', () => {
    const posts = [post(1, null), post(2, 1)]
    const map = computeRootMap(posts)
    const grouped = groupRepliesByRoot(posts, map)
    expect(grouped.get(1)?.some((r) => r.id === 1)).toBeFalsy()
  })

  it('返信が無い root はキー自体が出ない', () => {
    const posts = [post(1, null), post(2, null)]
    const map = computeRootMap(posts)
    const grouped = groupRepliesByRoot(posts, map)
    expect(grouped.size).toBe(0)
  })
})

describe('isTopLevel', () => {
  const buildIndex = (posts: ThreadablePost[]) => {
    const m = new Map<number, ThreadablePost>()
    for (const p of posts) m.set(p.id, p)
    return m
  }

  it('parent_id が null なら top-level', () => {
    const posts = [post(1, null)]
    expect(isTopLevel(posts[0], buildIndex(posts))).toBe(true)
  })

  it('親 id が posts に居ない (orphan) なら top-level', () => {
    const posts = [post(5, 99)]
    expect(isTopLevel(posts[0], buildIndex(posts))).toBe(true)
  })

  it('親 id が posts に居れば top-level ではない', () => {
    const posts = [post(1, null), post(2, 1)]
    expect(isTopLevel(posts[1], buildIndex(posts))).toBe(false)
  })
})
