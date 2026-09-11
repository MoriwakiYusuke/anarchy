import '@testing-library/jest-dom';
import { TextEncoder, TextDecoder } from 'util';

// Polyfill for TextEncoder/TextDecoder (Node.js環境用)
global.TextEncoder = TextEncoder;
global.TextDecoder = TextDecoder as typeof global.TextDecoder;

// window 依存のモックは jsdom 環境のみ (worker/ のテストは @jest-environment node で走る)
if (typeof window !== 'undefined') {
  // Mock matchMedia for useReducedMotion hook tests
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: jest.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: jest.fn(),
      removeListener: jest.fn(),
      addEventListener: jest.fn(),
      removeEventListener: jest.fn(),
      dispatchEvent: jest.fn(),
    })),
  });

  // Mock localStorage for i18n persistence tests
  const localStorageMock = {
    getItem: jest.fn(),
    setItem: jest.fn(),
    removeItem: jest.fn(),
    clear: jest.fn(),
  };
  Object.defineProperty(window, 'localStorage', {
    value: localStorageMock,
  });
}

jest.mock('@/hooks/useNicknameOf', () => ({
  useNicknameOf: () => null,
  // useNickname の成功パスが呼ぶキャッシュ無効化関数。モックに含めないと
  // 「invalidateNicknameCache is not a function」で success 遷移が error に化ける。
  invalidateNicknameCache: jest.fn(),
}));

// AccountContext は内部で polkadot-api signer (ESM) を import するため Jest が
// 解析に失敗する。テストでは own account 情報を使わないので最小スタブを返す。
jest.mock('@/lib/account/context', () => ({
  useAccount: () => ({ account: null, signer: null, mainRawSigner: null, setAccount: () => {} }),
}));

// Mock i18n so components using `useLocale` work without a provider.
// Uses ja translations directly so existing tests can assert on Japanese text.
// eslint-disable-next-line @typescript-eslint/no-var-requires
const jaTranslations = require('./src/i18n/translations/ja.json');
jest.mock('@/i18n', () => ({
  useLocale: () => ({
    locale: 'ja' as const,
    setLocale: jest.fn(),
    t: (key: string, params?: Record<string, string | number>) => {
      const template: string = jaTranslations[key] ?? key;
      if (!params) return template;
      return Object.entries(params).reduce(
        (acc, [k, v]) => acc.replace(new RegExp(`\\{${k}\\}`, 'g'), String(v)),
        template,
      );
    },
    availableLocales: [],
  }),
  LocaleProvider: ({ children }: { children: React.ReactNode }) => children,
  DEFAULT_LOCALE: 'ja',
  SUPPORTED_LOCALES: ['en', 'ja', 'zh'],
}));

// jsdom 環境には structuredClone が無い。fake-indexeddb (postContentCache テスト) が
// 値の格納時に呼ぶので、Node の v8 serializer で代替する。
if (typeof globalThis.structuredClone !== 'function') {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const v8 = require('node:v8');
  globalThis.structuredClone = <T>(value: T): T => v8.deserialize(v8.serialize(value));
}
