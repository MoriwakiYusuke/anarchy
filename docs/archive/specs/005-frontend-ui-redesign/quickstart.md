# Quickstart: Frontend UI Redesign

**Feature**: 005-frontend-ui-redesign  
**Date**: 2026-02-08

## Prerequisites

- Node.js 20+
- pnpm (recommended) or npm
- 既存のAnarchyフロントエンド環境

## 1. 開発環境セットアップ

```bash
# リポジトリルートから
cd apps/frontend
pnpm install
```

## 2. 開発サーバー起動

```bash
pnpm dev
# http://localhost:3000 でアクセス
```

## 3. 実装順序

### Step 1: i18n基盤

```bash
# 1. 型定義を作成
mkdir -p src/i18n
touch src/i18n/types.ts

# 2. 翻訳ファイルを作成
mkdir -p src/i18n/translations
touch src/i18n/translations/{en,ja,zh}.json

# 3. Contextを実装
touch src/i18n/context.tsx

# 4. フックを実装
touch src/hooks/useLocale.ts
```

### Step 2: 言語切替UI

```bash
# 1. コンポーネント作成
touch src/components/LanguageSwitcher.tsx
touch src/components/LanguageSwitcher.module.css

# 2. layout.tsxに統合
```

### Step 3: 既存コンポーネントの国際化

```bash
# 各コンポーネントのハードコード文字列をt()に置換
# WalletConnect.tsx, PostForm.tsx, Timeline.tsx
```

### Step 4: Matrix背景

```bash
# 1. アニメーションロジック
mkdir -p src/lib/matrix
touch src/lib/matrix/{index,config,types}.ts

# 2. フック
touch src/hooks/useReducedMotion.ts

# 3. コンポーネント
touch src/components/MatrixBackground.tsx
touch src/components/MatrixBackground.module.css

# 4. layout.tsxに統合
```

## 4. 動作確認チェックリスト

### 多言語対応 (✅ 実装完了)

- [x] ブラウザ言語に基づいて初期言語が設定される
- [x] 言語切替ボタンで3言語（EN/JA/ZH）を切り替えられる
- [x] ページリロード後も選択言語が維持される（localStorage）
- [x] 全UI要素が翻訳されている（WalletConnect, PostForm, Timeline）

**テスト方法:**
```bash
pnpm test tests/hooks/useLocale.test.tsx
pnpm test tests/components/LanguageSwitcher.test.tsx
```

### cMatrix背景 (✅ 実装完了)

- [x] ページ読み込み時にアニメーションが開始される
- [x] 文字がダークグレー（#333333）で落下する
- [x] 約2%の確率で赤い文字（Blood Glitch #CC0000）が出現する
- [x] コンテンツの可読性が損なわれていない（z-index: -1）
- [x] ウィンドウリサイズで正しく再描画される

**テスト方法:**
```bash
pnpm test tests/components/MatrixBackground.test.tsx
pnpm test tests/lib/matrix.test.ts
```

### アクセシビリティ (✅ 実装完了)

- [x] `prefers-reduced-motion: reduce`設定でアニメーションが停止する
- [x] キーボードで言語切替が操作できる
- [x] WCAG 2.1 AA コントラスト比準拠（全て4.5:1以上）

**テスト方法:**
```bash
pnpm test tests/hooks/useReducedMotion.test.ts
# ブラウザでprefers-reduced-motionをtoggleして確認
```

### WCAG Contrast Ratios (検証済み)

| 組み合わせ | コントラスト比 | 要件 |
|-----------|--------------|------|
| 白(#fff)/黒(#000) | 21.00:1 | ✅ PASS |
| 白(#fff)/BG(#0a0a0a) | 19.80:1 | ✅ PASS |
| 副テキスト(#888)/黒 | 5.92:1 | ✅ PASS |
| アクセント(#ff4444)/黒 | 6.16:1 | ✅ PASS |

## 5. トラブルシューティング

### 言語が保存されない

```javascript
// localStorage確認
console.log(localStorage.getItem('anarchy-locale'));

// プライベートブラウズモードではlocalStorageが制限される場合あり
```

### 背景が表示されない

```javascript
// Canvas APIサポート確認
console.log(!!document.createElement('canvas').getContext);

// z-indexの競合確認
// MatrixBackgroundは z-index: -1 で描画
```

### アニメーションがカクつく

```javascript
// デバイスパフォーマンス確認
// intervalMsを70-80msに調整

// 開発者ツールのPerformanceタブでプロファイリング
```

## 6. テスト実行

```bash
# 全ユニットテスト（49件）
pnpm test

# 個別テスト
pnpm test tests/hooks/useLocale.test.tsx
pnpm test tests/hooks/useReducedMotion.test.ts
pnpm test tests/components/LanguageSwitcher.test.tsx
pnpm test tests/components/MatrixBackground.test.tsx
pnpm test tests/lib/matrix.test.ts

# カバレッジ
pnpm test -- --coverage
```

## 7. ビルド確認

```bash
pnpm build
pnpm start
# http://localhost:3000 で本番ビルドを確認
```

## 8. 翻訳追加方法

新しいUI要素を追加する場合：

1. `src/i18n/types.ts` の `TranslationKey` に新しいキーを追加
2. 各翻訳ファイル（en.json, ja.json, zh.json）にエントリを追加
3. コンポーネントで `t('new.key')` を使用

```typescript
// types.ts
type TranslationKey = 
  | ... existing keys
  | 'new.feature.label';

// en.json
{
  "new.feature.label": "New Feature"
}

// Component.tsx
const { t } = useLocale();
<span>{t('new.feature.label')}</span>
```

## 関連ドキュメント

- [spec.md](spec.md) - 機能仕様
- [data-model.md](data-model.md) - データモデル定義
- [contracts/i18n-api.md](contracts/i18n-api.md) - APIコントラクト
- [research.md](research.md) - 技術選定理由
