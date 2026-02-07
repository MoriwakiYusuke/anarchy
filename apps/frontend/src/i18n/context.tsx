'use client';

/**
 * LocaleContext Provider
 * 
 * Provides i18n context for the entire application.
 * Handles locale state management and persistence.
 */

import React, { createContext, useContext, useState, useCallback, useEffect, useMemo } from 'react';
import type { PropsWithChildren } from 'react';
import type { Locale, TranslationKey, TranslationMap, LocaleContextValue } from './types';
import { DEFAULT_LOCALE, LOCALE_STORAGE_KEY, isValidLocale, LOCALE_CONFIGS } from './types';

// Import translations
import enTranslations from './translations/en.json';
import jaTranslations from './translations/ja.json';
import zhTranslations from './translations/zh.json';

/**
 * All translations mapped by locale
 */
const translations: Record<Locale, TranslationMap> = {
  en: enTranslations as TranslationMap,
  ja: jaTranslations as TranslationMap,
  zh: zhTranslations as TranslationMap,
};

/**
 * Locale context - undefined when not inside provider
 */
const LocaleContext = createContext<LocaleContextValue | undefined>(undefined);

/**
 * Get initial locale from localStorage or default
 */
function getInitialLocale(): Locale {
  if (typeof window === 'undefined') return DEFAULT_LOCALE;
  
  try {
    const saved = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (saved && isValidLocale(saved)) {
      return saved;
    }
  } catch {
    // localStorage not available or error - use default
  }
  
  return DEFAULT_LOCALE;
}

/**
 * LocaleProvider Props
 */
interface LocaleProviderProps extends PropsWithChildren {
  initialLocale?: Locale;
}

/**
 * LocaleProvider Component
 * 
 * Wrap your application with this provider to enable i18n.
 * 
 * @example
 * ```tsx
 * <LocaleProvider>
 *   <App />
 * </LocaleProvider>
 * ```
 */
export function LocaleProvider({ children, initialLocale }: LocaleProviderProps) {
  const [locale, setLocaleState] = useState<Locale>(initialLocale ?? DEFAULT_LOCALE);
  const [isHydrated, setIsHydrated] = useState(false);

  // Hydrate from localStorage on client
  useEffect(() => {
    if (!isHydrated) {
      const savedLocale = getInitialLocale();
      setLocaleState(savedLocale);
      setIsHydrated(true);
    }
  }, [isHydrated]);

  // Persist locale changes to localStorage
  const setLocale = useCallback((newLocale: Locale) => {
    setLocaleState(newLocale);
    try {
      localStorage.setItem(LOCALE_STORAGE_KEY, newLocale);
    } catch {
      // localStorage not available - ignore
    }
  }, []);

  /**
   * Translation function with parameter interpolation
   */
  const t = useCallback((key: TranslationKey, params?: Record<string, string | number>): string => {
    const translation = translations[locale]?.[key];
    
    if (!translation) {
      // Return key as fallback for missing translations
      console.warn(`Missing translation for key: ${key} in locale: ${locale}`);
      return key;
    }

    if (!params) return translation;

    // Interpolate parameters: {param} -> value
    return translation.replace(/\{(\w+)\}/g, (_, paramName) => {
      const value = params[paramName];
      return value !== undefined ? String(value) : `{${paramName}}`;
    });
  }, [locale]);

  /**
   * Available locales with their configurations
   */
  const availableLocales = useMemo(() => Object.values(LOCALE_CONFIGS), []);

  const contextValue = useMemo<LocaleContextValue & { availableLocales: typeof availableLocales }>(
    () => ({
      locale,
      setLocale,
      t,
      availableLocales,
    }),
    [locale, setLocale, t, availableLocales]
  );

  return (
    <LocaleContext.Provider value={contextValue}>
      {children}
    </LocaleContext.Provider>
  );
}

/**
 * useLocale Hook
 * 
 * Access i18n context within a LocaleProvider.
 * 
 * @throws Error if used outside LocaleProvider
 * 
 * @example
 * ```tsx
 * const { locale, setLocale, t } = useLocale();
 * t('wallet.connect') // "Connect Wallet"
 * ```
 */
export function useLocale(): LocaleContextValue & { availableLocales: { code: Locale; displayName: string; nativeName: string }[] } {
  const context = useContext(LocaleContext);
  
  if (context === undefined) {
    throw new Error('useLocale must be used within a LocaleProvider');
  }
  
  return context as LocaleContextValue & { availableLocales: { code: Locale; displayName: string; nativeName: string }[] };
}
