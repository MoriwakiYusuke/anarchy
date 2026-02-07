/**
 * Unit Tests for useLocale Hook
 * 
 * TDD: Written BEFORE implementation - tests should FAIL initially
 */

import { renderHook, act } from '@testing-library/react';
import React from 'react';
import type { PropsWithChildren } from 'react';

// These will be implemented
import { useLocale, LocaleProvider } from '@/i18n';
import type { Locale } from '@/i18n/types';

describe('useLocale Hook', () => {
  const wrapper = ({ children }: PropsWithChildren) => (
    <LocaleProvider>{children}</LocaleProvider>
  );

  beforeEach(() => {
    localStorage.clear();
    jest.clearAllMocks();
  });

  describe('Initial State', () => {
    it('should default to English locale', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      expect(result.current.locale).toBe('en');
    });

    it('should load saved locale from localStorage', () => {
      (localStorage.getItem as jest.Mock).mockReturnValue('ja');
      const { result } = renderHook(() => useLocale(), { wrapper });
      expect(result.current.locale).toBe('ja');
    });

    it('should ignore invalid saved locale', () => {
      (localStorage.getItem as jest.Mock).mockReturnValue('invalid');
      const { result } = renderHook(() => useLocale(), { wrapper });
      expect(result.current.locale).toBe('en');
    });
  });

  describe('setLocale', () => {
    it('should change locale to Japanese', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      
      act(() => {
        result.current.setLocale('ja');
      });

      expect(result.current.locale).toBe('ja');
    });

    it('should change locale to Chinese', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      
      act(() => {
        result.current.setLocale('zh');
      });

      expect(result.current.locale).toBe('zh');
    });

    it('should persist locale to localStorage', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      
      act(() => {
        result.current.setLocale('ja');
      });

      expect(localStorage.setItem).toHaveBeenCalledWith('anarchy-locale', 'ja');
    });
  });

  describe('t (translation function)', () => {
    it('should return English translation for nav.home', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      expect(result.current.t('nav.home')).toBe('Home');
    });

    it('should return Japanese translation after locale change', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      
      act(() => {
        result.current.setLocale('ja');
      });

      expect(result.current.t('nav.home')).toBe('ホーム');
    });

    it('should return Chinese translation after locale change', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      
      act(() => {
        result.current.setLocale('zh');
      });

      expect(result.current.t('nav.home')).toBe('首页');
    });

    it('should return key if translation is missing', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      // @ts-expect-error testing invalid key
      expect(result.current.t('missing.key')).toBe('missing.key');
    });
  });

  describe('availableLocales', () => {
    it('should return all supported locales', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      expect(result.current.availableLocales).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ code: 'en' }),
          expect.objectContaining({ code: 'ja' }),
          expect.objectContaining({ code: 'zh' }),
        ])
      );
    });

    it('should include native names for each locale', () => {
      const { result } = renderHook(() => useLocale(), { wrapper });
      
      const enLocale = result.current.availableLocales.find(l => l.code === 'en');
      const jaLocale = result.current.availableLocales.find(l => l.code === 'ja');
      const zhLocale = result.current.availableLocales.find(l => l.code === 'zh');

      expect(enLocale?.nativeName).toBe('English');
      expect(jaLocale?.nativeName).toBe('日本語');
      expect(zhLocale?.nativeName).toBe('中文');
    });
  });

  describe('Error Handling', () => {
    it('should throw error if used outside LocaleProvider', () => {
      // Suppress console.error for this test
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation();
      
      expect(() => renderHook(() => useLocale())).toThrow(
        'useLocale must be used within a LocaleProvider'
      );

      consoleSpy.mockRestore();
    });
  });
});
