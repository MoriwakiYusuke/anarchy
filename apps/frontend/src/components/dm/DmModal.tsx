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
import { useSmoldot } from '@/hooks/useSmoldot';
import { useAccount } from '@/lib/account/context';
import { stealthKeyManager } from '@/lib/stealth/keyManager';
import { getDmMetaAddressFromStealth } from '@/lib/dm/keyManager';
import { initSs58Toolkit, type ScanContext } from '@/lib/dm/scanner';
import type { SendDmContext, StorageSigner } from '@/lib/dm/sender';
import { sendDmReceipt } from '@/lib/dm/receipt';
import { startDmScanLoop, type DmScanLoopHandle } from '@/lib/dm/worker';
import { useDmStore } from '@/lib/dm/store';
import { exportDmBackup, importDmBackup } from '@/lib/dm/keyManager';
import { ConversationList } from './ConversationList';
import { ConversationView } from './ConversationView';
import { DmKeyManager } from './DmKeyManager';
import { BlockListManager } from './BlockListManager';
import { MissingBackupNotice } from './MissingBackupNotice';
import type { AccountId } from '@/lib/dm/types';
import styles from './DmModal.module.css';

export interface DmModalProps {
  isOpen: boolean;
  onClose: () => void;
}

type Tab = 'inbox' | 'settings';

export function DmModal({ isOpen, onClose }: DmModalProps): JSX.Element | null {
  const { unsafeApi } = useSmoldot();
  const { account, accountSeed, signer } = useAccount();
  const [mounted, setMounted] = useState(false);
  const [activeTab, setActiveTab] = useState<Tab>('inbox');
  const [selected, setSelected] = useState<AccountId | null>(null);
  const [newRecipient, setNewRecipient] = useState('');
  const [mainRawSigner, setMainRawSigner] = useState<StorageSigner | null>(null);
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

  // 接続中アカウントの seed phrase / derive path から raw sr25519 signer を作る。
  useEffect(() => {
    if (!accountSeed) {
      setMainRawSigner(null);
      return;
    }
    let cancelled = false;
    void (async () => {
      const { cryptoWaitReady } = await import('@polkadot/util-crypto');
      await cryptoWaitReady();
      const { Keyring } = await import('@polkadot/keyring');
      const keyring = new Keyring({ type: 'sr25519' });
      const pair = keyring.addFromUri(accountSeed);
      if (cancelled) return;
      setMainRawSigner({
        publicKey: pair.publicKey,
        sign: (msg: Uint8Array) => pair.sign(msg),
      });
    })();
    return () => {
      cancelled = true;
    };
  }, [accountSeed]);

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
        ).catch((err) => console.error('[dm-receipt][delivered]', err));
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
          ウォレットを接続してください。
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
              {isScanning ? '● スキャン中' : '○ 待機中'}
            </span>
            <span className={styles.blockNumber}>#{lastScannedBlock.toString()}</span>
          </div>
          <form className={styles.composeRow} onSubmit={handleNewDm}>
            <input
              type="text"
              className={styles.composeInput}
              placeholder="相手の SS58 アドレス (5...)"
              value={newRecipient}
              onChange={(e) => setNewRecipient(e.target.value)}
            />
            <button
              type="submit"
              className={styles.composeBtn}
              disabled={!newRecipient.trim()}
            >
              開く
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
              {selected ? '接続中…' : 'スレッドを選択するか、新しい DM を始めてください。'}
            </div>
          )}
        </section>
      </div>
    );
  };

  const renderSettings = (): ReactNode => {
    if (!account) {
      return <div className={styles.notice}>ウォレットを接続してください。</div>;
    }
    return (
      <DmSettings
        hasStealthKey={keyLoaded}
        unsafeApi={unsafeApi}
        signer={signer}
      />
    );
  };

  return createPortal(
    <div
      className={styles.overlay}
      role="dialog"
      aria-modal="true"
      aria-label="ダイレクトメッセージ"
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
              受信箱
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={activeTab === 'settings'}
              className={activeTab === 'settings' ? styles.tabActive : styles.tab}
              onClick={() => setActiveTab('settings')}
            >
              設定
            </button>
          </div>
          <button
            type="button"
            className={styles.closeBtn}
            onClick={onClose}
            aria-label="閉じる"
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
}

function DmSettings({ hasStealthKey, unsafeApi, signer }: DmSettingsProps): JSX.Element {
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
      setStatus({
        kind: 'ok',
        message: 'stealth 鍵を生成しました。続けて DM 鍵を公開できます。',
      });
    } catch (err) {
      setStatus({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setIsGenerating(false);
    }
  }, []);

  const handleExport = async (): Promise<void> => {
    try {
      if (!password) {
        setStatus({ kind: 'error', message: 'パスワードを入力してください。' });
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
      setStatus({ kind: 'ok', message: 'バックアップをダウンロードしました。' });
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
        setStatus({ kind: 'error', message: 'パスワードを入力してください。' });
        return;
      }
      const buf = new Uint8Array(await file.arrayBuffer());
      await importDmBackup(buf, password);
      setStatus({ kind: 'ok', message: 'バックアップをインポートしました。' });
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
        <section className={styles.section} aria-label="鍵を準備">
          <h3 className={styles.sectionTitle}>鍵を準備</h3>
          <p className={styles.description}>
            DM の暗号化にはステルスアドレス用の鍵ペアが必要です。鍵はこのブラウザの
            メモリ内だけで管理し、ページを閉じると破棄されます。新しく作るか、既存の
            バックアップファイルから復元してください。
          </p>
          <div className={styles.keyChoice}>
            <div className={styles.keyChoiceItem}>
              <h4 className={styles.choiceLabel}>新規発行</h4>
              <p className={styles.choiceHelp}>このブラウザ用に新しい鍵ペアを生成します。</p>
              <button
                type="button"
                className={styles.primaryBtn}
                onClick={() => void handleGenerate()}
                disabled={isGenerating}
              >
                {isGenerating ? '生成中…' : '新しい鍵を生成'}
              </button>
            </div>
            <div className={styles.keyChoiceDivider} aria-hidden="true">または</div>
            <div className={styles.keyChoiceItem}>
              <h4 className={styles.choiceLabel}>バックアップから復元</h4>
              <p className={styles.choiceHelp}>
                以前エクスポートした暗号化バックアップを読み込みます。
              </p>
              <label className={styles.field}>
                <span className={styles.label}>パスワード</span>
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className={styles.input}
                  placeholder="バックアップ作成時のパスワード"
                />
              </label>
              <label className={styles.fileLabel}>
                <span>バックアップファイルを選択</span>
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
          <section className={styles.section} aria-label="DM 鍵管理">
            {unsafeApi && signer ? (
              <DmKeyManager
                api={unsafeApi as Parameters<typeof DmKeyManager>[0]['api']}
                signer={signer}
                initialPublished={false}
              />
            ) : (
              <p className={styles.muted}>接続中…</p>
            )}
          </section>

          <section className={styles.section} aria-label="バックアップ書き出し">
            <h3 className={styles.sectionTitle}>バックアップ書き出し</h3>
            <p className={styles.description}>
              DM 鍵と会話履歴をパスワードで暗号化したファイルとして保存します
              (FR-022)。別の端末でインポートすれば DM を引き継げます。
            </p>
            <label className={styles.field}>
              <span className={styles.label}>パスワード</span>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className={styles.input}
                placeholder="8 文字以上を推奨"
              />
            </label>
            <button
              type="button"
              className={styles.primaryBtn}
              onClick={() => void handleExport()}
              disabled={!password}
            >
              バックアップを保存
            </button>
          </section>

          <section className={styles.section} aria-label="ブロックリスト">
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
          エラー: {status.message}
        </p>
      )}
    </div>
  );
}

export default DmModal;
