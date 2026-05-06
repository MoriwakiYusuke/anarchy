'use client';

/**
 * DmModal — DM 機能を 1 つのモーダルに集約。/dm/* ページの代替。
 *
 * 設計:
 * - 上部ヘッダ: タイトル + close ボタン + tab スイッチ (受信箱 / 設定)
 * - 受信箱タブ:
 *   - スレッド一覧 (master) + 選択中の会話 (detail) を 2 ペイン
 *   - 会話未選択 / モバイル幅では一覧のみ表示
 * - 設定タブ:
 *   - 鍵を準備 / DM 受信鍵 publish/revoke / バックアップ書き出し / ブロックリスト
 *   - /dm/settings の構造をそのまま流用
 *
 * Scanner ループはモーダルが開いている間のみ動作。閉じている間は CPU を使わない
 * (バックグラウンド scan が必要になったら別途 provider 化する)。
 */

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { useChain } from '@/hooks/useChain';
import { useAccount } from '@/lib/account/context';
import { stealthKeyManager } from '@/lib/stealth/keyManager';
import { getDmMetaAddressFromStealth } from '@/lib/dm/keyManager';
import { initSs58Toolkit, type ScanContext } from '@/lib/dm/scanner';
import type { SendDmContext } from '@/lib/dm/sender';
import { sendDmReceipt } from '@/lib/dm/receipt';
import { startDmScanLoop, type DmScanLoopHandle } from '@/lib/dm/worker';
import { debugError } from '@/lib/debugLog';
import { useDmStore } from '@/lib/dm/store';
import { exportDmBackup, importDmBackup } from '@/lib/dm/keyManager';
import { ConversationList } from './ConversationList';
import { ConversationView } from './ConversationView';
import { DmKeyManager } from './DmKeyManager';
import { BlockListManager } from './BlockListManager';
import { MissingBackupNotice } from './MissingBackupNotice';
import { useLocale } from '@/i18n';
import type { AccountId } from '@/lib/dm/types';
import styles from './DmModal.module.css';

export interface DmModalProps {
  isOpen: boolean;
  onClose: () => void;
}

type Tab = 'inbox' | 'settings';

export function DmModal({ isOpen, onClose }: DmModalProps): JSX.Element | null {
  const { t } = useLocale();
  const { unsafeApi } = useChain();
  const { account, signer, mainRawSigner } = useAccount();
  const [mounted, setMounted] = useState(false);
  const [activeTab, setActiveTab] = useState<Tab>('inbox');
  const [selected, setSelected] = useState<AccountId | null>(null);
  const [newRecipient, setNewRecipient] = useState('');
  const [keyLoaded, setKeyLoaded] = useState(false);
  const loopRef = useRef<DmScanLoopHandle | null>(null);

  const isScanning = useDmStore((s: { isScanning: boolean }) => s.isScanning);
  const lastScannedBlock = useDmStore(
    (s: { lastScannedBlock: bigint }) => s.lastScannedBlock,
  );

  useEffect(() => {
    setMounted(true);
  }, []);

  // ESC で閉じる。
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [isOpen, onClose]);

  // mainRawSigner は AccountContext が seed 受領直後に keyring pair を作って
  // expose してくれているので、ここでは追加の derive 不要。

  // stealth 鍵のロード状態を polling で監視 (session memory)。
  useEffect(() => {
    const check = (): void =>
      setKeyLoaded(stealthKeyManager.getMetaAddress() !== null);
    check();
    const id = window.setInterval(check, 1_000);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    void initSs58Toolkit();
  }, []);

  // 受信ループ: モーダルが開いている + 鍵 + api + signer + mainRawSigner で起動。
  useEffect(() => {
    if (!isOpen || !keyLoaded || !unsafeApi || !signer || !mainRawSigner) return;
    const sendCtx: SendDmContext = {
      api: unsafeApi,
      mainSigner: signer,
      mainAccountPublicKey: new Uint8Array(signer.publicKey),
      mainRawSigner,
    };
    const handle = startDmScanLoop({
      buildContext: (): ScanContext | null => {
        const meta = getDmMetaAddressFromStealth();
        const scanPriv = stealthKeyManager.getViewKey();
        if (!meta || !scanPriv) return null;
        return {
          api: unsafeApi,
          ownScanPriv: new Uint8Array(scanPriv),
          ownSpendPub: meta.spendPub,
          ownMainAccount: '' as AccountId,
          lastScannedBlock: useDmStore.getState().lastScannedBlock,
        };
      },
      onNewIncoming: (msg) => {
        void sendDmReceipt(
          { counterparty: msg.counterparty, refMessageId: msg.messageId, kind: 'delivered' },
          sendCtx,
        ).catch((err) => debugError('[dm-receipt][delivered]', err));
      },
    });
    loopRef.current = handle;
    return () => {
      handle.stop();
      loopRef.current = null;
    };
  }, [isOpen, keyLoaded, unsafeApi, signer, mainRawSigner]);

  const sendCtx: SendDmContext | null = useMemo(() => {
    if (!unsafeApi || !signer) return null;
    return {
      api: unsafeApi,
      mainSigner: signer,
      mainAccountPublicKey: new Uint8Array(signer.publicKey),
      mainRawSigner: mainRawSigner ?? undefined,
    };
  }, [unsafeApi, signer, mainRawSigner]);

  const handleOpenConversation = useCallback((counterparty: AccountId) => {
    setSelected(counterparty);
  }, []);

  const handleNewDm = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const trimmed = newRecipient.trim();
      if (!trimmed) return;
      setSelected(trimmed as AccountId);
      setNewRecipient('');
    },
    [newRecipient],
  );

  if (!mounted || !isOpen) return null;

  const renderInbox = (): ReactNode => {
    if (!account) {
      return (
        <div className={styles.notice}>
          {t('dm.modal.connectWallet')}
        </div>
      );
    }
    if (!keyLoaded) {
      return <MissingBackupNotice onOpenSettings={() => setActiveTab('settings')} />;
    }
    return (
      <div className={styles.inbox}>
        <aside className={styles.threadPane}>
          <div className={styles.statusRow}>
            <span className={isScanning ? styles.scanning : styles.idle}>
              {isScanning ? t('dm.status.scanning') : t('dm.status.idle')}
            </span>
            <span className={styles.blockNumber}>#{lastScannedBlock.toString()}</span>
          </div>
          <form className={styles.composeRow} onSubmit={handleNewDm}>
            <input
              type="text"
              className={styles.composeInput}
              placeholder={t('dm.compose.newDmPlaceholder')}
              value={newRecipient}
              onChange={(e) => setNewRecipient(e.target.value)}
            />
            <button
              type="submit"
              className={styles.composeBtn}
              disabled={!newRecipient.trim()}
            >
              {t('dm.compose.openButton')}
            </button>
          </form>
          <div className={styles.listScroll}>
            <ConversationList onSelect={handleOpenConversation} />
          </div>
        </aside>
        <section className={styles.conversationPane}>
          {selected && sendCtx ? (
            <ConversationView conversationId={selected} context={sendCtx} />
          ) : (
            <div className={styles.empty}>
              {selected ? t('dm.connecting') : t('dm.modal.selectThread')}
            </div>
          )}
        </section>
      </div>
    );
  };

  const renderSettings = (): ReactNode => {
    if (!account) {
      return <div className={styles.notice}>{t('dm.modal.connectWallet')}</div>;
    }
    return (
      <DmSettings
        hasStealthKey={keyLoaded}
        unsafeApi={unsafeApi}
        signer={signer}
        accountId={account}
      />
    );
  };

  return createPortal(
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label={t('dm.modal.ariaLabel')}
      onClick={onClose}
    >
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <header className={styles.header}>
          <h2 className={styles.title}>
            <span className={styles.accent}>D</span>irect Messages
          </h2>
          <div className={styles.tabs} role="tablist">
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === 'inbox'}
              className={activeTab === 'inbox' ? styles.tabActive : styles.tab}
              onClick={() => setActiveTab('inbox')}
            >
              {t('dm.modal.tab.inbox')}
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === 'settings'}
              className={activeTab === 'settings' ? styles.tabActive : styles.tab}
              onClick={() => setActiveTab('settings')}
            >
              {t('dm.modal.tab.settings')}
            </button>
          </div>
          <button
            type="button"
            className={styles.closeBtn}
            onClick={onClose}
            aria-label={t('dm.modal.close')}
          >
            ✕
          </button>
        </header>
        <div className={styles.body}>
          {activeTab === 'inbox' ? renderInbox() : renderSettings()}
        </div>
      </div>
    </div>,
    document.body,
  );
}

/* ============================================================ */
/* Settings sub-component (内部だけで使うので別ファイルに分けない)  */
/* ============================================================ */

interface DmSettingsProps {
  hasStealthKey: boolean;
  unsafeApi: unknown;
  signer: import('polkadot-api/signer').PolkadotSigner | null;
  accountId: string | null;
}

function DmSettings({ hasStealthKey, unsafeApi, signer, accountId }: DmSettingsProps): JSX.Element {
  const { t } = useLocale();
  const [password, setPassword] = useState('');
  const [status, setStatus] = useState<
    { kind: 'idle' } | { kind: 'ok'; message: string } | { kind: 'error'; message: string }
  >({ kind: 'idle' });
  const [isGenerating, setIsGenerating] = useState(false);

  const handleGenerate = useCallback(async () => {
    setIsGenerating(true);
    setStatus({ kind: 'idle' });
    try {
      await stealthKeyManager.generateKeys();
      setStatus({ kind: 'ok', message: t('dm.settings.stealthGenerated') });
    } catch (err) {
      setStatus({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setIsGenerating(false);
    }
  }, [t]);

  const handleExport = async (): Promise<void> => {
    try {
      if (!password) {
        setStatus({ kind: 'error', message: t('dm.settings.passwordRequired') });
        return;
      }
      const bytes = await exportDmBackup(password);
      const blob = new Blob([bytes.buffer as ArrayBuffer], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `anarchy-dm-backup-${Date.now()}.bin`;
      a.click();
      URL.revokeObjectURL(url);
      setStatus({ kind: 'ok', message: t('dm.settings.exportSection.success') });
    } catch (err) {
      setStatus({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  const handleImport = async (file: File): Promise<void> => {
    try {
      if (!password) {
        setStatus({ kind: 'error', message: t('dm.settings.passwordRequired') });
        return;
      }
      const buf = new Uint8Array(await file.arrayBuffer());
      await importDmBackup(buf, password);
      setStatus({ kind: 'ok', message: t('dm.settings.importSuccess') });
    } catch (err) {
      setStatus({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  return (
    <div className={styles.settings}>
      {!hasStealthKey ? (
        <section className={styles.section} aria-label={t('dm.settings.prepareKey.title')}>
          <h3 className={styles.sectionTitle}>{t('dm.settings.prepareKey.title')}</h3>
          <p className={styles.description}>{t('dm.settings.prepareKey.description')}</p>
          <div className={styles.keyChoice}>
            <div className={styles.keyChoiceItem}>
              <h4 className={styles.choiceLabel}>{t('dm.settings.prepareKey.newLabel')}</h4>
              <p className={styles.choiceHelp}>{t('dm.settings.prepareKey.newHelp')}</p>
              <button
                type="button"
                className={styles.primaryBtn}
                onClick={() => void handleGenerate()}
                disabled={isGenerating}
              >
                {isGenerating
                  ? t('dm.settings.prepareKey.generating')
                  : t('dm.settings.prepareKey.generateButton')}
              </button>
            </div>
            <div className={styles.keyChoiceDivider} aria-hidden="true">{t('dm.settings.prepareKey.or')}</div>
            <div className={styles.keyChoiceItem}>
              <h4 className={styles.choiceLabel}>{t('dm.settings.prepareKey.restoreLabel')}</h4>
              <p className={styles.choiceHelp}>{t('dm.settings.prepareKey.restoreHelp')}</p>
              <label className={styles.field}>
                <span className={styles.label}>{t('dm.settings.prepareKey.passwordLabel')}</span>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className={styles.input}
                  placeholder={t('dm.settings.prepareKey.passwordPlaceholder')}
                />
              </label>
              <label className={styles.fileLabel}>
                <span>{t('dm.settings.prepareKey.fileLabel')}</span>
                <input
                  type="file"
                  accept=".bin,application/octet-stream"
                  onChange={(e) => {
                    const f = e.target.files?.[0];
                    if (f) void handleImport(f);
                  }}
                  className={styles.fileInput}
                />
              </label>
            </div>
          </div>
        </section>
      ) : (
        <>
          <section className={styles.section} aria-label={t('dm.keyManager.title')}>
            {unsafeApi && signer ? (
              <DmKeyManager
                api={unsafeApi as Parameters<typeof DmKeyManager>[0]['api']}
                signer={signer}
                accountId={accountId ?? undefined}
              />
            ) : (
              <p className={styles.muted}>{t('dm.connecting')}</p>
            )}
          </section>

          <section className={styles.section} aria-label={t('dm.settings.exportSection.title')}>
            <h3 className={styles.sectionTitle}>{t('dm.settings.exportSection.title')}</h3>
            <p className={styles.description}>{t('dm.settings.exportSection.description')}</p>
            <label className={styles.field}>
              <span className={styles.label}>{t('dm.settings.prepareKey.passwordLabel')}</span>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className={styles.input}
                placeholder={t('dm.settings.exportSection.passwordPlaceholder')}
              />
            </label>
            <button
              type="button"
              className={styles.primaryBtn}
              onClick={() => void handleExport()}
              disabled={!password}
            >
              {t('dm.settings.exportSection.button')}
            </button>
          </section>

          <section className={styles.section} aria-label={t('dm.blockList.title')}>
            <BlockListManager />
          </section>
        </>
      )}

      {status.kind === 'ok' && (
        <p role="status" aria-live="polite" className={styles.success}>
          {status.message}
        </p>
      )}
      {status.kind === 'error' && (
        <p role="alert" className={styles.error}>
          {t('dm.settings.errorPrefix', { detail: status.message })}
        </p>
      )}
    </div>
  );
}

export default DmModal;
