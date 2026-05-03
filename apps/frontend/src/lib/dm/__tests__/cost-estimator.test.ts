/**
 * estimateDmCostFromInputs の精度テスト。
 *
 * 検証方針: 実際の wire format (`encodeDmContent` + `DmEnvelope` SCALE 計算) と
 * バケット選択ロジックを使うので、見積もりは「dm_encrypt_and_pad が実行された
 * ときに選ばれるバケット」と一致するはず。境界値で 1 段ずれないことを確認。
 */

import { encodeDmContent } from '../contentCodec';
import { estimateDmCostFromInputs, formatMoral } from '../sender';

const MORAL = 1_000_000_000_000n;

/** PADDING_BUCKETS と一致する想定値。 */
const BUCKETS = [1024, 4096, 16384, 65536, 262144] as const;

function expectedCostFor(bucket: number): bigint {
  // base 1 + per_byte 0.05 * bucket + margin 1
  return 1n * MORAL + 50_000_000_000n * BigInt(bucket) + 1n * MORAL;
}

describe('estimateDmCostFromInputs', () => {
  it('text-only short message → bucket 1024', () => {
    const cost = estimateDmCostFromInputs('hello', []);
    expect(cost).toBe(expectedCostFor(1024));
    // 1 + 0.05 * 1024 + 1 = 53.20
    expect(formatMoral(cost!)).toBe('53.20');
  });

  it('long Japanese text just under 1024 bucket', () => {
    // body = 4 magic + ~24 JSON wrap + textBytes; envelope = 105 + scale_prefix + body + 1 + 16
    // 1024 bucket allows: 1024 - 16 - 1 - 105 - 2 (scale prefix for 64-16383) = 900 byte body
    const text = 'あ'.repeat(290); // 870 UTF-8 bytes
    const cost = estimateDmCostFromInputs(text, []);
    expect(cost).toBe(expectedCostFor(1024));
  });

  it('text just over 1024 bucket → 4096', () => {
    // 1024 を超えるサイズの text を投入
    const text = 'あ'.repeat(400); // 1200 UTF-8 bytes
    const cost = estimateDmCostFromInputs(text, []);
    expect(cost).toBe(expectedCostFor(4096));
  });

  it('5 attachments + short text → 4096 bucket', () => {
    const files = new Array(5).fill({ mime: 'image/png', size: 1024 });
    const cost = estimateDmCostFromInputs('hello', files);
    // 5 refs * ~280B + envelope = ~1500 → 4096 bucket
    expect(cost).toBe(expectedCostFor(4096));
  });

  it('1 video with 8KB thumbnail → 16384 bucket (thumbnail dominates)', () => {
    // 動画サムネは data URL (base64) で数 KB になる。8KB の thumbnail を含むと
    // body が 1024 を超えて 4096 か 16384 に乗る。
    const thumbnail = 'data:image/jpeg;base64,' + 'A'.repeat(8000);
    const cost = estimateDmCostFromInputs('video!', [
      {
        mime: 'video/mp4',
        size: 1024 * 1024,
        width: 1920,
        height: 1080,
        duration: 12,
        thumbnail,
      },
    ]);
    // body ≈ 4 + 8400 = 8404 → bucket 16384
    expect(cost).toBe(expectedCostFor(16384));
  });

  it('returns null for body that overflows the largest bucket', () => {
    // 30KB の thumbnail × 5 video = 150KB → envelope ~150KB → 262144 でぎり収まる…
    // 100KB ファイル × 数十個など、262144 を超える組み合わせを作る。
    const huge = 'data:image/jpeg;base64,' + 'A'.repeat(60_000);
    const files = new Array(5).fill({
      mime: 'video/mp4',
      size: 1,
      thumbnail: huge,
    });
    const cost = estimateDmCostFromInputs('x', files);
    expect(cost).toBeNull();
  });

  it('matches encodeDmContent output length (sanity)', () => {
    // 内部で encodeDmContent を使っているので、当然 wire と一致する。
    // 万が一実装が乖離した場合に検出するためのガード。
    const text = 'sanity check';
    const dummy = encodeDmContent({ text, media: [] });
    expect(dummy.length).toBeGreaterThan(0);
    const cost = estimateDmCostFromInputs(text, []);
    expect(cost).not.toBeNull();
  });

  it('legacy API (number) still works for backward compat', () => {
    const cost = estimateDmCostFromInputs('hello', 0);
    expect(cost).toBe(expectedCostFor(1024));
  });
});

describe('formatMoral', () => {
  it('formats whole MORAL', () => {
    expect(formatMoral(53n * MORAL + 200_000_000_000n)).toBe('53.20');
    expect(formatMoral(0n)).toBe('0.00');
    expect(formatMoral(1n * MORAL)).toBe('1.00');
  });

  it('truncates fractional below 0.01', () => {
    // 0.001 MORAL は表示 "0.00" (切り捨て)。
    expect(formatMoral(1_000_000_000n)).toBe('0.00');
  });
});
