/**
 * i18n Type Definitions
 * 
 * Defines types for internationalization support in Anarchy frontend.
 */

/**
 * Supported locale identifiers
 */
export type Locale = 'en' | 'ja' | 'zh';

/**
 * Default locale when no preference is detected
 */
export const DEFAULT_LOCALE: Locale = 'en';

/**
 * All supported locales
 */
export const SUPPORTED_LOCALES: readonly Locale[] = ['en', 'ja', 'zh'] as const;

/**
 * Configuration for each locale
 */
export interface LocaleConfig {
  code: Locale;
  displayName: string;  // Display name in that language
  nativeName: string;   // Native name
}

/**
 * Locale configurations
 */
export const LOCALE_CONFIGS: Record<Locale, LocaleConfig> = {
  en: { code: 'en', displayName: 'English', nativeName: 'English' },
  ja: { code: 'ja', displayName: 'Japanese', nativeName: '日本語' },
  zh: { code: 'zh', displayName: 'Chinese', nativeName: '中文' },
};

/**
 * Translation keys - type-safe string literals for all translatable text
 */
export type TranslationKey =
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
  | 'wallet.title'
  | 'wallet.selectAccount'
  | 'wallet.refreshBalance'
  | 'wallet.copy'
  | 'wallet.copied'
  | 'wallet.seedErrorEmpty'
  | 'wallet.seedErrorInvalid'
  | 'wallet.generate'
  | 'wallet.seedPhrase'
  | 'wallet.dev'
  | 'wallet.devTestAccount'
  | 'wallet.seedHint'
  | 'wallet.seedNote'
  | 'wallet.seedWarning'
  // Post
  | 'post.placeholder'
  | 'post.submit'
  | 'post.submitting'
  | 'post.cost'
  | 'post.empty'
  | 'post.sending'
  | 'post.uploading'
  | 'post.splitting'
  | 'post.recording'
  | 'post.success'
  | 'post.defaultCostNote'
  // Content
  | 'content.loading'
  | 'content.error'
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
  | 'balance.insufficient'
  // Language Switcher
  | 'language.select'
  | 'language.current'
  // App
  | 'app.subtitle'
  | 'app.connected'
  | 'app.disconnected'
  | 'app.syncing'
  | 'app.connecting'
  // Faucet
  | 'faucet.button'
  | 'faucet.mining'
  | 'faucet.submitting'
  | 'faucet.success'
  | 'faucet.error'
  | 'faucet.cancel'
  // Error messages
  | 'error.contentTooLong'
  | 'error.tooManyPosts'
  | 'error.parentPostNotFound'
  | 'error.insufficientMoralBalance'
  | 'error.insufficientBalance'
  | 'error.overflow'
  | 'error.selfTransfer'
  | 'error.exhaustsResources'
  | 'error.invalidTransaction'
  | 'error.payment'
  | 'error.badOrigin'
  | 'error.moduleError'
  | 'error.unknown'
  | 'error.alreadyClaimed'
  | 'error.challengeExpired'
  | 'error.invalidProof'
  | 'error.blockNotFound'
  | 'error.faucetNetworkError'
  // Transfer
  | 'transfer.title'
  | 'transfer.recipient'
  | 'transfer.recipientPlaceholder'
  | 'transfer.amount'
  | 'transfer.amountPlaceholder'
  | 'transfer.send'
  | 'transfer.sending'
  | 'transfer.confirm'
  | 'transfer.confirmTitle'
  | 'transfer.confirmMessage'
  | 'transfer.cancel'
  | 'transfer.balance'
  | 'transfer.success'
  | 'transfer.error'
  | 'error.invalidRecipient'
  | 'error.emptyRecipient'
  | 'error.invalidAddressLength'
  | 'error.invalidAddressCharacter'
  | 'error.invalidAddressPrefix'
  | 'error.emptyAmount'
  | 'error.invalidAmount'
  | 'error.amountTooSmall'
  | 'error.amountExceedsBalance'
  | 'error.missingDependencies'
  | 'error.invalidTransferState'
  | 'error.transferFailed'
  | 'error.existentialDeposit'
  | 'error.networkTimeout'
  // Address
  | 'address.copy'
  | 'address.copied'
  | 'address.copyFailed'
  | 'address.clickToCopy'
  // Name
  | 'name.label'
  | 'name.change'
  // Nickname
  | 'nickname.title'
  | 'nickname.set'
  | 'nickname.clear'
  | 'nickname.placeholder'
  | 'nickname.setting'
  | 'nickname.success'
  | 'nickname.cleared'
  | 'nickname.error'
  | 'error.nicknameTooLong'
  | 'error.nicknameEmpty'
  // Media
  | 'media.upload'
  | 'media.uploading'
  | 'media.dropzone'
  | 'media.remove'
  | 'media.retry'
  | 'media.retrying'
  | 'media.retryAll'
  | 'media.clearAll'
  | 'media.uploadFailed'
  | 'media.processing'
  | 'media.complete'
  | 'media.loadError'
  | 'error.fileTooLarge'
  | 'error.unsupportedFileType'
  | 'error.tooManyFiles'
  | 'error.uploadFailed'
  | 'error.videoNotSupported';

/**
 * Translation map - maps translation keys to translated strings
 */
export type TranslationMap = Record<TranslationKey, string>;

/**
 * Context value provided by LocaleProvider
 */
export interface LocaleContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: TranslationKey, params?: Record<string, string | number>) => string;
}

/**
 * localStorage key for persisting locale preference
 */
export const LOCALE_STORAGE_KEY = 'anarchy-locale';

/**
 * Check if a string is a valid locale
 */
export function isValidLocale(value: string): value is Locale {
  return SUPPORTED_LOCALES.includes(value as Locale);
}
