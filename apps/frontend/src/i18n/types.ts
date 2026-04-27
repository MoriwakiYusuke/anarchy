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
  | 'nav.dm'
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
  | 'post.expand'
  | 'post.collapse'
  // Reaction
  | 'reaction.pleaseConnect'
  | 'reaction.alreadyReacted'
  | 'reaction.submitting'
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
  | 'faucet.submitted'
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
  | 'error.nicknameSetFailed'
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
  | 'error.videoNotSupported'
  | 'error.nicknameQueryFailed'
  | 'error.nicknameClearFailed'
  | 'error.signerRequired'
  | 'error.apiNotReady'
  | 'error.emptyFile'
  // Stealth
  | 'stealth.title'
  | 'stealth.receive'
  | 'stealth.send'
  | 'stealth.balance'
  | 'stealth.generate.title'
  | 'stealth.generate.description'
  | 'stealth.generate.button'
  | 'stealth.generate.generating'
  | 'stealth.generate.error'
  | 'stealth.metaAddress.title'
  | 'stealth.metaAddress.label'
  | 'stealth.metaAddress.yourAddress'
  | 'stealth.metaAddress.shareHint'
  | 'stealth.metaAddress.copy'
  | 'stealth.backup.title'
  | 'stealth.backup.warning'
  | 'stealth.backup.passwordPlaceholder'
  | 'stealth.backup.confirmPlaceholder'
  | 'stealth.backup.download'
  | 'stealth.backup.complete'
  | 'stealth.backup.createError'
  | 'stealth.backup.passwordTooShort'
  | 'stealth.backup.passwordMismatch'
  | 'stealth.import.title'
  | 'stealth.import.button'
  | 'stealth.import.selectFile'
  | 'stealth.import.password'
  | 'stealth.import.passwordPlaceholder'
  | 'stealth.import.importing'
  | 'stealth.import.importButton'
  | 'stealth.import.cancel'
  | 'stealth.import.noFile'
  | 'stealth.import.noPassword'
  | 'stealth.import.error'
  | 'stealth.manage.title'
  | 'stealth.manage.export'
  | 'stealth.manage.exporting'
  | 'stealth.manage.clear'
  | 'stealth.manage.or'
  | 'stealth.sendForm.recipientLabel'
  | 'stealth.sendForm.amountLabel'
  | 'stealth.sendForm.submit'
  | 'stealth.sendForm.submitting'
  | 'stealth.sendForm.sending'
  | 'stealth.sendForm.success'
  | 'stealth.sendForm.error'
  | 'stealth.sendForm.description'
  | 'stealth.sendForm.pageTitle'
  | 'stealth.sendForm.connecting'
  | 'stealth.sendForm.notReady'
  | 'stealth.sendForm.requireSignIn'
  | 'stealth.sendForm.error.metaAddressRequired'
  | 'stealth.sendForm.error.metaAddressPrefix'
  | 'stealth.sendForm.error.metaAddressFormat'
  | 'stealth.sendForm.error.metaAddressBase58'
  | 'stealth.sendForm.error.metaAddressLength'
  | 'stealth.sendForm.error.invalidAddress'
  | 'stealth.sendForm.error.invalidAmount'
  | 'stealth.scan.title'
  | 'stealth.scan.total'
  | 'stealth.scan.start'
  | 'stealth.scan.stop'
  | 'stealth.scan.scanning'
  | 'stealth.scan.block'
  | 'stealth.scan.detected'
  | 'stealth.scan.empty'
  | 'stealth.scan.emptyHint'
  | 'stealth.scan.needKeys'
  | 'stealth.spend.title'
  | 'stealth.spend.selectAll'
  | 'stealth.spend.clearSelection'
  | 'stealth.spend.noBalance'
  | 'stealth.spend.selected'
  | 'stealth.spend.amountLabel'
  | 'stealth.spend.autoSelect'
  | 'stealth.spend.recipientLabel'
  | 'stealth.spend.submit'
  | 'stealth.spend.submitting'
  | 'stealth.spend.cancel'
  | 'stealth.spend.sendBalance'
  | 'stealth.spend.success'
  | 'stealth.spend.failed'
  | 'stealth.spend.error.selectBalance'
  | 'stealth.spend.error.spentIncluded'
  | 'stealth.spend.error.recipientRequired'
  | 'stealth.spend.error.amountRequired'
  | 'stealth.spend.error.insufficientBalance'
  | 'stealth.spend.error.invalidAmount'
  | 'stealth.spend.error.checkInput'
  | 'stealth.linkability.title'
  | 'stealth.linkability.description'
  | 'stealth.linkability.confirm'
  | 'stealth.linkability.cancel'
  | 'stealth.balance.spent'
  | 'stealth.balance.title'
  | 'stealth.balance.description'
  | 'stealth.balance.noMetaAddress'
  | 'stealth.balance.backToList'
  | 'stealth.button'
  // DM (019-direct-messages)
  | 'dm.title'
  | 'dm.nav.home'
  | 'dm.nav.settings'
  | 'dm.nav.backToInbox'
  | 'dm.status.scanning'
  | 'dm.status.idle'
  | 'dm.status.lastBlock'
  | 'dm.compose.newDmLabel'
  | 'dm.compose.newDmPlaceholder'
  | 'dm.compose.openButton'
  | 'dm.compose.bodyPlaceholder'
  | 'dm.compose.send'
  | 'dm.compose.sending'
  | 'dm.compose.sent'
  | 'dm.compose.retry'
  | 'dm.compose.attachButton'
  | 'dm.compose.attachmentsLabel'
  | 'dm.compose.removeAttachment'
  | 'dm.compose.estimatedCost'
  | 'dm.compose.bodyTooLargeWarn'
  | 'dm.progress.attaching'
  | 'dm.progress.encrypting'
  | 'dm.progress.uploading'
  | 'dm.progress.uploadingProgress'
  | 'dm.progress.prefunding'
  | 'dm.progress.dispatching'
  | 'dm.progress.done'
  | 'dm.error.recipientKeyNotPublished'
  | 'dm.error.insufficientBalance'
  | 'dm.error.storageInsufficient'
  | 'dm.error.transactionDropped'
  | 'dm.error.bodyTooLarge'
  | 'dm.error.mediaUploadFailed'
  | 'dm.error.generic'
  | 'dm.media.loading'
  | 'dm.media.fetchFailed'
  | 'dm.media.downloadFile'
  | 'dm.media.openImage'
  | 'dm.list.empty'
  | 'dm.list.unreadAriaLabel'
  | 'dm.view.counterpartyLabel'
  | 'dm.view.selfLabel'
  | 'dm.view.selfName'
  | 'dm.view.empty'
  | 'dm.view.deliveryStateAriaLabel'
  | 'dm.view.deliveryStateSent'
  | 'dm.view.deliveryStateDelivered'
  | 'dm.view.deliveryStateRead'
  | 'dm.view.gcPlaceholder'
  | 'dm.missingKey.title'
  | 'dm.missingKey.description'
  | 'dm.missingKey.openSettings'
  | 'dm.redirecting'
  | 'dm.connecting'
  | 'dm.settings.title'
  | 'dm.settings.prepareKey.title'
  | 'dm.settings.prepareKey.description'
  | 'dm.settings.prepareKey.newLabel'
  | 'dm.settings.prepareKey.newHelp'
  | 'dm.settings.prepareKey.generateButton'
  | 'dm.settings.prepareKey.generating'
  | 'dm.settings.prepareKey.or'
  | 'dm.settings.prepareKey.restoreLabel'
  | 'dm.settings.prepareKey.restoreHelp'
  | 'dm.settings.prepareKey.passwordLabel'
  | 'dm.settings.prepareKey.passwordPlaceholder'
  | 'dm.settings.prepareKey.fileLabel'
  | 'dm.settings.stealthGenerated'
  | 'dm.settings.passwordRequired'
  | 'dm.settings.exportSection.title'
  | 'dm.settings.exportSection.description'
  | 'dm.settings.exportSection.passwordPlaceholder'
  | 'dm.settings.exportSection.button'
  | 'dm.settings.exportSection.success'
  | 'dm.settings.importSuccess'
  | 'dm.settings.errorPrefix'
  | 'dm.keyManager.title'
  | 'dm.keyManager.statusLabel'
  | 'dm.keyManager.statusPublished'
  | 'dm.keyManager.statusUnpublished'
  | 'dm.keyManager.stealthMissing'
  | 'dm.keyManager.publish'
  | 'dm.keyManager.publishing'
  | 'dm.keyManager.revoke'
  | 'dm.keyManager.revoking'
  | 'dm.keyManager.publishSuccess'
  | 'dm.keyManager.revokeSuccess'
  | 'dm.keyManager.stealthRequired'
  | 'dm.keyManager.errorPrefix'
  | 'dm.keyManager.receiptFieldset'
  | 'dm.keyManager.receiptLegend'
  | 'dm.keyManager.receiptOptOut'
  | 'dm.blockList.title'
  | 'dm.blockList.empty'
  | 'dm.blockList.unblock'
  | 'dm.blockList.addressLabel'
  | 'dm.blockList.addressPlaceholder'
  | 'dm.blockList.addButton'
  // DmModal extras
  | 'dm.modal.ariaLabel'
  | 'dm.modal.close'
  | 'dm.modal.tab.inbox'
  | 'dm.modal.tab.settings'
  | 'dm.modal.connectWallet'
  | 'dm.modal.selectThread';

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
