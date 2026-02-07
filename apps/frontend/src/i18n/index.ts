/**
 * i18n Module Exports
 * 
 * Central export point for all i18n functionality.
 */

// Types
export type { 
  Locale, 
  TranslationKey, 
  TranslationMap, 
  LocaleContextValue,
  LocaleConfig,
} from './types';

// Constants
export { 
  DEFAULT_LOCALE, 
  SUPPORTED_LOCALES, 
  LOCALE_CONFIGS,
  LOCALE_STORAGE_KEY,
  isValidLocale,
} from './types';

// Context and Hook
export { LocaleProvider, useLocale } from './context';
