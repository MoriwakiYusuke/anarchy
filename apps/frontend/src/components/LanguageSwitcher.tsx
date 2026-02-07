'use client';

/**
 * LanguageSwitcher Component
 * 
 * Allows users to switch between supported languages (EN, JA, ZH).
 */

import React from 'react';
import { useLocale } from '@/i18n';
import type { Locale } from '@/i18n';
import styles from './LanguageSwitcher.module.css';

export interface LanguageSwitcherProps {
  /** Display variant - 'full' shows full names, 'compact' shows codes */
  variant?: 'full' | 'compact';
  /** Additional CSS class */
  className?: string;
}

/**
 * LanguageSwitcher Component
 * 
 * @example
 * ```tsx
 * <LanguageSwitcher variant="compact" />
 * ```
 */
export function LanguageSwitcher({ 
  variant = 'full', 
  className = '' 
}: LanguageSwitcherProps) {
  const { locale, setLocale, availableLocales, t } = useLocale();

  const handleLocaleChange = (newLocale: Locale) => {
    setLocale(newLocale);
  };

  const containerClass = [
    styles.container,
    styles[variant],
    className,
  ].filter(Boolean).join(' ');

  return (
    <div 
      className={containerClass}
      role="group" 
      aria-label={t('language.select')}
    >
      {availableLocales.map((localeConfig) => {
        const isActive = localeConfig.code === locale;
        const buttonClass = [
          styles.button,
          isActive && styles.active,
        ].filter(Boolean).join(' ');

        return (
          <button
            key={localeConfig.code}
            type="button"
            className={buttonClass}
            onClick={() => handleLocaleChange(localeConfig.code)}
            aria-current={isActive ? 'true' : undefined}
            aria-label={`${localeConfig.nativeName}${isActive ? ' (current)' : ''}`}
          >
            {variant === 'compact' 
              ? localeConfig.code.toUpperCase() 
              : localeConfig.nativeName
            }
          </button>
        );
      })}
    </div>
  );
}

export default LanguageSwitcher;
