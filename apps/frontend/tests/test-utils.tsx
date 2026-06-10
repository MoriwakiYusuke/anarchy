/**
 * 共有テストユーティリティ。
 *
 * 背景: jest.setup.ts は `@/i18n` をグローバルにモックしている (locale 固定 'ja')
 * が、一部コンポーネント (StealthModal / StealthBalanceList / StealthSpendForm 等)
 * は `@/i18n/context` を直接 import するためモックを素通りし、実物の useLocale が
 * 「useLocale must be used within a LocaleProvider」を投げる。
 *
 * このユーティリティは実物の LocaleProvider (`@/i18n/context`) で包んで render
 * する。locale は localStorage モック経由で注入する (Provider が mount 後の
 * hydration で localStorage から読み直すため、initialLocale だけでは不十分)。
 */

import React from 'react';
import { render, type RenderOptions, type RenderResult } from '@testing-library/react';
// `@/i18n` はグローバルモックされているため、実物は context / types から直接取る。
import { LocaleProvider } from '@/i18n/context';
import { LOCALE_STORAGE_KEY, type Locale } from '@/i18n/types';

export interface RenderWithLocaleOptions extends Omit<RenderOptions, 'wrapper'> {
  /** render する locale。既存テストの多くは日本語文言を assert するため既定 'ja'。 */
  locale?: Locale;
}

/**
 * 実物の LocaleProvider で包んで render する。
 */
export function renderWithLocale(
  ui: React.ReactElement,
  { locale = 'ja', ...options }: RenderWithLocaleOptions = {},
): RenderResult {
  // jest.setup.ts の localStorage モックに locale を仕込む。
  const getItem = window.localStorage.getItem as jest.Mock;
  if (typeof getItem?.mockImplementation === 'function') {
    getItem.mockImplementation((key: string) =>
      key === LOCALE_STORAGE_KEY ? locale : null,
    );
  }
  return render(<LocaleProvider initialLocale={locale}>{ui}</LocaleProvider>, options);
}
