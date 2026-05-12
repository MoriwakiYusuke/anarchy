# Data Model: Frontend UI Redesign

**Feature**: 005-frontend-ui-redesign  
**Date**: 2026-02-08

## Entities

### 1. Locale

言語識別子。サポートされる3言語のユニオン型。

```typescript
type Locale = 'en' | 'ja' | 'zh';
```

**Constraints**:
- 値は厳密に3種類のみ
- デフォルト値: `'en'`

### 2. LocaleConfig

各言語の設定情報。

```typescript
interface LocaleConfig {
  code: Locale;           // 'en' | 'ja' | 'zh'
  displayName: string;    // 表示名（その言語での表記）
  nativeName: string;     // ネイティブ名
}
```

**インスタンス**:

| code | displayName | nativeName |
|------|-------------|------------|
| en   | English     | English    |
| ja   | Japanese    | 日本語     |
| zh   | Chinese     | 中文       |

### 3. TranslationKey

翻訳キーの型安全な定義。ネームスペース別に整理。

```typescript
type TranslationKey = 
  // Navigation
  | 'nav.home'
  | 'nav.about'
  // Wallet
  | 'wallet.connect'
  | 'wallet.disconnect'
  | 'wallet.connecting'
  | 'wallet.connected'
  | 'wallet.enterSeed'
  | 'wallet.seedPlaceholder'
  // Post
  | 'post.placeholder'
  | 'post.submit'
  | 'post.submitting'
  | 'post.cost'
  | 'post.empty'
  // Timeline
  | 'timeline.empty'
  | 'timeline.loading'
  | 'timeline.error'
  // Common
  | 'common.error'
  | 'common.success'
  | 'common.loading'
  | 'common.retry'
  // Balance
  | 'balance.label'
  | 'balance.insufficient';
```

### 4. TranslationMap

言語ごとの翻訳データ構造。

```typescript
type TranslationMap = Record<TranslationKey, string>;
```

### 5. LocaleContextValue

Context APIで提供される値の型。

```typescript
interface LocaleContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: TranslationKey, params?: Record<string, string | number>) => string;
}
```

### 6. MatrixConfig

cMatrix背景アニメーションの設定。

```typescript
interface MatrixConfig {
  // Colors
  mainColor: string;      // '#333333' - 基本文字色
  headColor: string;      // '#999999' - 先頭文字色
  glitchColor: string;    // '#CC0000' - Blood Glitch色
  trailAlpha: number;     // 0.05 - 残像の透明度
  
  // Animation
  intervalMs: number;     // 60 - 更新間隔（ミリ秒）
  glitchProbability: number; // 0.02 - グリッチ発生確率（2%）
  
  // Characters
  charset: string;        // 使用する文字セット
  fontSize: number;       // 14 - フォントサイズ（px）
  
  // Performance
  enabled: boolean;       // アニメーション有効/無効
}
```

**デフォルト値**:

```typescript
const DEFAULT_MATRIX_CONFIG: MatrixConfig = {
  mainColor: '#333333',
  headColor: '#999999',
  glitchColor: '#CC0000',
  trailAlpha: 0.05,
  intervalMs: 60,
  glitchProbability: 0.02,
  charset: 'アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ',
  fontSize: 14,
  enabled: true,
};
```

### 7. MatrixColumn

各列の状態を管理する内部データ構造。

```typescript
interface MatrixColumn {
  x: number;              // X座標（px）
  y: number;              // 現在のY座標（px）
  speed: number;          // 落下速度（1-3の範囲）
}
```

## State Management

### LocaleContext State

```typescript
// 永続化キー
const STORAGE_KEY = 'anarchy-locale';

// 初期値の決定ロジック
function getInitialLocale(): Locale {
  // 1. localStorage確認
  // 2. navigator.languages確認
  // 3. デフォルト 'en'
}
```

### MatrixBackground State

```typescript
// Canvas ref
canvasRef: RefObject<HTMLCanvasElement>

// Animation state
columns: MatrixColumn[]
intervalId: number | null

// Derived
isEnabled: boolean // prefers-reduced-motion考慮
```

## Relationships

```
┌─────────────────┐
│  LocaleContext  │
│  (Provider)     │
├─────────────────┤
│ - locale        │──────────┐
│ - setLocale()   │          │
│ - t()           │          ▼
└─────────────────┘    ┌─────────────────┐
        │              │ TranslationMap  │
        │              │ (JSON files)    │
        ▼              └─────────────────┘
┌─────────────────┐           │
│ LanguageSwitcher│◄──────────┘
│ (Component)     │
└─────────────────┘

┌─────────────────┐
│ MatrixBackground│
│ (Component)     │
├─────────────────┤
│ - config        │
│ - canvasRef     │
│ - columns[]     │
└─────────────────┘
        │
        ▼
┌─────────────────┐
│ useReducedMotion│
│ (Hook)          │
└─────────────────┘
```

## Validation Rules

| Entity | Rule | Error Handling |
|--------|------|----------------|
| Locale | 'en' \| 'ja' \| 'zh' のみ | 無効値は 'en' にフォールバック |
| TranslationKey | 存在するキーのみ | 未定義キーはキー自体を返す |
| MatrixConfig.intervalMs | 50-80の範囲 | 範囲外は60にクランプ |
| MatrixConfig.glitchProbability | 0-1の範囲 | 範囲外は0.02にクランプ |
