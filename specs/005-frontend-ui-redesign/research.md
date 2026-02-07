# Research: Frontend UI Redesign

**Feature**: 005-frontend-ui-redesign  
**Date**: 2026-02-08  
**Purpose**: Phase 0における技術選定とベストプラクティスの調査結果

## 1. 多言語対応（i18n）アプローチ

### 選択肢の比較

| アプローチ | メリット | デメリット |
|-----------|---------|-----------|
| **next-intl** | Next.js 14 App Router完全対応、Server Components対応 | 追加依存、学習コスト |
| **react-i18next** | 豊富なエコシステム、実績 | SSR設定が複雑、App Router対応が不完全 |
| **Custom Context** | 依存なし、完全制御可能、軽量 | 機能が限定的、pluralization等は自前実装 |

### 決定: Custom Context + JSON

**Rationale**:
- 翻訳対象が約50箇所と小規模であり、外部ライブラリのオーバーヘッドが不要
- シンプルなキー・値マッピングで十分（pluralization、日付フォーマット等は不要）
- バンドルサイズを最小限に抑えたい
- Next.js 14のuse clientディレクティブと相性が良い

**却下理由**:
- next-intl: 機能が過剰、依存追加が不必要
- react-i18next: App Router対応が複雑、設定コストが高い

### 実装方針

```typescript
// LocaleContext で言語状態を管理
// localStorage で永続化
// navigator.language で初期検出
// JSON ファイルで翻訳データを管理
```

## 2. cMatrix背景アニメーション

### Canvas APIベストプラクティス

#### パフォーマンス最適化

1. **requestAnimationFrame vs setInterval**
   - **決定**: setIntervalを使用（50-80ms間隔）
   - **理由**: cMatrixは60fpsである必要がなく、意図的に遅いアニメーションが「大人のアナーキー」感を演出

2. **Canvas描画最適化**
   - フルスクリーンCanvasはfixed positionでbody直下に配置
   - 透明度を使った残像効果（`fillRect`でオーバーレイ）
   - 文字描画は`fillText`で最小限に

3. **メモリ管理**
   - 列ごとのy座標配列を事前確保
   - 文字セットは定数として保持
   - コンポーネントアンマウント時にintervalをクリア

#### Blood Glitch実装

```typescript
// 約2%の確率で赤色
const isGlitch = Math.random() > 0.98;
ctx.fillStyle = isGlitch ? '#CC0000' : '#333333';
```

### モバイル対応

- デバイスのpixelRatioを考慮したCanvas解像度
- 列数をウィンドウ幅に応じて動的計算
- リサイズイベントでCanvas再初期化

## 3. アクセシビリティ

### prefers-reduced-motion

```typescript
// カスタムフック
const useReducedMotion = () => {
  const [reducedMotion, setReducedMotion] = useState(false);
  
  useEffect(() => {
    const query = window.matchMedia('(prefers-reduced-motion: reduce)');
    setReducedMotion(query.matches);
    
    const handler = (e: MediaQueryListEvent) => setReducedMotion(e.matches);
    query.addEventListener('change', handler);
    return () => query.removeEventListener('change', handler);
  }, []);
  
  return reducedMotion;
};
```

### WCAG 2.1 AA準拠

- コンテンツエリアは不透明背景（`--bg-primary`）で覆う
- 背景アニメーションはコンテンツの「外側」または「隙間」からのみ視認
- 文字色のコントラスト比は既存デザインを維持

## 4. 言語検出ロジック

### 優先順位

1. localStorage に保存された設定
2. `navigator.languages[0]` のプレフィックス検出
3. `navigator.language` のプレフィックス検出
4. デフォルト: `'en'`

### マッピング

```typescript
const detectLocale = (): Locale => {
  const stored = localStorage.getItem('anarchy-locale');
  if (stored && ['en', 'ja', 'zh'].includes(stored)) return stored as Locale;
  
  const browserLangs = navigator.languages || [navigator.language];
  for (const lang of browserLangs) {
    const prefix = lang.split('-')[0].toLowerCase();
    if (prefix === 'ja') return 'ja';
    if (prefix === 'zh') return 'zh';
    if (prefix === 'en') return 'en';
  }
  return 'en';
};
```

## 5. バンドルサイズ考慮

### 翻訳ファイルのロード戦略

- **決定**: 静的インポート（全言語をバンドルに含める）
- **理由**: 
  - 3言語×50キー程度で合計約5KB以下
  - 動的インポートのオーバーヘッドより静的インポートが軽量
  - オフライン対応が容易

### Canvas描画コード

- ライブラリ不使用、純粋なCanvas API
- 追加バンドルサイズ: 約2KB（圧縮後）

## 結論

| 項目 | 決定 | 追加依存 |
|------|------|----------|
| i18n | Custom Context + JSON | なし |
| アニメーション | Canvas API + setInterval | なし |
| 状態管理 | React Context | なし |
| 永続化 | localStorage | なし |

**総合評価**: 外部依存なしで全要件を実装可能。シンプルで保守性の高い設計。
