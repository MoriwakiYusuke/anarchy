import {
  DM_CONTENT_MAGIC,
  decodeDmContent,
  encodeDmContent,
  isDmContentEnvelope,
} from '../contentCodec';
import { encodeReceiptBody, RECEIPT_MAGIC } from '../receipt';
import type { DmMediaRef } from '../types';

describe('DM contentCodec', () => {
  it('encodes/decodes plain text envelope (empty media[])', () => {
    const encoded = encodeDmContent({ text: 'hello world', media: [] });
    expect(isDmContentEnvelope(encoded)).toBe(true);
    expect(encoded[0]).toBe(DM_CONTENT_MAGIC[0]);

    const decoded = decodeDmContent(encoded);
    expect(decoded).not.toBeNull();
    expect(decoded!.text).toBe('hello world');
    expect(decoded!.media).toEqual([]);
  });

  it('roundtrips text + media refs', () => {
    const ref: DmMediaRef = {
      root: 'a'.repeat(64),
      key: 'b'.repeat(64),
      mime: 'image/png',
      size: 1234,
      k: 3,
      n: 5,
      ciphertextLen: 4096,
      width: 800,
      height: 600,
    };
    const encoded = encodeDmContent({ text: 'see attached', media: [ref] });
    const decoded = decodeDmContent(encoded);
    expect(decoded).not.toBeNull();
    expect(decoded!.text).toBe('see attached');
    expect(decoded!.media).toEqual([ref]);
  });

  it('rejects body without DMC magic', () => {
    const raw = new TextEncoder().encode('plain utf-8 message');
    expect(isDmContentEnvelope(raw)).toBe(false);
    expect(decodeDmContent(raw)).toBeNull();
  });

  it('does not collide with receipt magic prefix', () => {
    // receipt magic は [0x00, 0x44, 0x4D, 0x52], content magic は [0x44, 0x4D, 0x43, 0x01]。
    // 先頭 byte が異なる (0x00 vs 0x44) ので receipt body を content として誤読しない。
    expect(RECEIPT_MAGIC[0]).not.toBe(DM_CONTENT_MAGIC[0]);
    const receipt = encodeReceiptBody('read', 42n);
    expect(isDmContentEnvelope(receipt)).toBe(false);
  });

  it('drops malformed media entries', () => {
    // root が hex 64 桁でない、key が無いなどのエントリは sanitize で除去。
    const json = JSON.stringify({
      text: 'mix',
      media: [
        { root: 'short', key: 'b'.repeat(64), mime: 'image/png', size: 1, k: 3, n: 5, ciphertextLen: 1 },
        { root: 'a'.repeat(64), key: 'b'.repeat(64), mime: 'image/png', size: 1, k: 3, n: 5, ciphertextLen: 1 },
      ],
    });
    const buf = new Uint8Array(DM_CONTENT_MAGIC.length + json.length);
    buf.set(DM_CONTENT_MAGIC, 0);
    buf.set(new TextEncoder().encode(json), DM_CONTENT_MAGIC.length);
    const decoded = decodeDmContent(buf);
    expect(decoded!.media).toHaveLength(1);
    expect(decoded!.media[0].root).toBe('a'.repeat(64));
  });

  it('returns null for invalid JSON after magic', () => {
    const broken = new Uint8Array(DM_CONTENT_MAGIC.length + 5);
    broken.set(DM_CONTENT_MAGIC, 0);
    broken.set([0xff, 0xff, 0xff, 0xff, 0xff], DM_CONTENT_MAGIC.length);
    expect(decodeDmContent(broken)).toBeNull();
  });
});
