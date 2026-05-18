# API Contract: i18n & Matrix Background

**Feature**: 005-frontend-ui-redesign  
**Date**: 2026-02-08  
**Type**: React Component & Hook APIs

## 1. LocaleContext API

### Provider

```tsx
interface LocaleProviderProps {
  children: React.ReactNode;
  defaultLocale?: Locale;  // optional, defaults to auto-detect
}

// Usage
<LocaleProvider>
  <App />
</LocaleProvider>
```

### Hook: useLocale

```typescript
function useLocale(): LocaleContextValue;

interface LocaleContextValue {
  /**
   * Current active locale
   */
  locale: Locale;
  
  /**
   * Change the active locale
   * @param locale - Target locale
   * Side effect: Updates localStorage
   */
  setLocale: (locale: Locale) => void;
  
  /**
   * Translate a key to the current locale
   * @param key - Translation key
   * @param params - Optional interpolation params
   * @returns Translated string, or key itself if not found
   * 
   * @example
   * t('post.cost', { amount: 10 }) // "Cost: 10 $moral"
   */
  t: (key: TranslationKey, params?: Record<string, string | number>) => string;
}
```

### Error Handling

| Scenario | Behavior |
|----------|----------|
| Unknown locale in localStorage | Reset to 'en', clear invalid storage |
| Translation key not found | Return key as-is, log warning in dev |
| localStorage unavailable | Session-only state, no persistence |

## 2. LanguageSwitcher Component

```tsx
interface LanguageSwitcherProps {
  /**
   * Visual variant
   * @default 'dropdown'
   */
  variant?: 'dropdown' | 'inline';
  
  /**
   * Additional CSS class
   */
  className?: string;
}

// Usage
<LanguageSwitcher />
<LanguageSwitcher variant="inline" />
```

### Accessibility

- `role="listbox"` for dropdown
- `aria-label="Select language"` (localized)
- Keyboard navigation support (Enter, Escape, Arrow keys)
- Focus management on open/close

## 3. MatrixBackground Component

```tsx
interface MatrixBackgroundProps {
  /**
   * Override default configuration
   */
  config?: Partial<MatrixConfig>;
  
  /**
   * Force disable animation (useful for testing)
   * @default false
   */
  disabled?: boolean;
}

// Usage
<MatrixBackground />
<MatrixBackground config={{ glitchProbability: 0.05 }} />
<MatrixBackground disabled />
```

### Lifecycle

| Event | Action |
|-------|--------|
| Mount | Initialize canvas, start animation loop |
| Unmount | Stop animation, clear interval, release canvas |
| Window resize | Recalculate columns, resize canvas |
| prefers-reduced-motion change | Toggle animation on/off |

### CSS Requirements

Component renders with the following structure:

```html
<canvas 
  class="matrix-background"
  style="position: fixed; top: 0; left: 0; z-index: -1; pointer-events: none;"
/>
```

## 4. useReducedMotion Hook

```typescript
/**
 * Detects user's motion preference
 * @returns true if user prefers reduced motion
 */
function useReducedMotion(): boolean;

// Usage
const reducedMotion = useReducedMotion();
if (reducedMotion) {
  // Skip animation
}
```

### Implementation Notes

- Uses `matchMedia('(prefers-reduced-motion: reduce)')`
- Subscribes to changes via `addEventListener('change', ...)`
- SSR-safe: returns `false` during server render

## 5. Translation File Format

### File Structure

```
src/i18n/translations/
├── en.json
├── ja.json
└── zh.json
```

### JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "nav.home",
    "wallet.connect",
    "post.placeholder"
    // ... all TranslationKey values
  ],
  "properties": {
    "nav.home": { "type": "string" },
    "wallet.connect": { "type": "string" },
    "post.placeholder": { "type": "string" }
    // ...
  }
}
```

### Interpolation Syntax

```json
{
  "post.cost": "Cost: {{amount}} $moral",
  "balance.label": "Balance: {{balance}}"
}
```

Pattern: `{{paramName}}` replaced at runtime.

## 6. Integration Points

### layout.tsx Updates

```tsx
// Before
export default function RootLayout({ children }) {
  return (
    <html lang="ja">
      <body>{children}</body>
    </html>
  );
}

// After
export default function RootLayout({ children }) {
  return (
    <html lang="en"> {/* Default, updated by client */}
      <body>
        <LocaleProvider>
          <MatrixBackground />
          {children}
        </LocaleProvider>
      </body>
    </html>
  );
}
```

### Component Migration

All UI text must be replaced with `t()` calls:

```tsx
// Before
<button>Connect Wallet</button>

// After
const { t } = useLocale();
<button>{t('wallet.connect')}</button>
```

## 7. Testing Contracts

### Unit Tests Required

| Component/Hook | Test Cases |
|----------------|------------|
| `useLocale` | Initial detection, setLocale persistence, t() interpolation |
| `useReducedMotion` | Default value, media query change |
| `LanguageSwitcher` | Render all options, selection change |
| `MatrixBackground` | Canvas initialization, disabled prop, cleanup |

### Integration Tests Required

| Scenario | Verification |
|----------|--------------|
| Full page with i18n | All text translates correctly |
| Language persistence | Reload maintains selection |
| Animation + content | Content remains readable |
| Mobile viewport | Canvas scales appropriately |
