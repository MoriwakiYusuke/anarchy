//! ISO 7816-4 padding + fixed-size bucket helpers for DM ciphertext.
//!
//! contracts/wasm-engine-dm-api.md §Non-API Helpers / data-model.md §2.2 に準拠。

/// DM ciphertext が取りうる許容サイズ (FR-026)。R4 の議論で 256B バケットは不採用。
pub const DM_PADDING_BUCKETS: [usize; 5] = [1_024, 4_096, 16_384, 65_536, 262_144];

/// AES-GCM tag のバイト長 (128-bit)。
pub const AES_GCM_TAG_LEN: usize = 16;

/// Envelope サイズ (padding + terminator) に tag を足した総 ciphertext 長が収まる
/// 最小のバケット値を返す。超過時は `None` (body too large)。
pub fn select_padding_bucket(padded_plaintext_len: usize) -> Option<usize> {
    DM_PADDING_BUCKETS
        .iter()
        .copied()
        .find(|&b| padded_plaintext_len + AES_GCM_TAG_LEN <= b)
}

/// ISO 7816-4 padding: 0x80 に続けて 0x00 を `target_len - input.len() - 1` 個追加。
/// 必ず terminator を挿入するため `input.len() < target_len` が前提で、満たさない
/// 場合は `None` を返す (呼出側で `JsError` 等にマップする想定)。
pub fn pad_iso7816_4(input: &[u8], target_len: usize) -> Option<Vec<u8>> {
    if input.len() >= target_len {
        return None;
    }
    let mut out = Vec::with_capacity(target_len);
    out.extend_from_slice(input);
    out.push(0x80);
    out.resize(target_len, 0x00);
    Some(out)
}

/// ISO 7816-4 padding 剥ぎ取り。末尾の `0x00*` を skip し、直近の `0x80` を境界と
/// する。`0x80` が見つからない / データが空の場合は `None`。
pub fn strip_iso7816_4(padded: &[u8]) -> Option<&[u8]> {
    let last_80 = padded.iter().rposition(|&b| b == 0x80)?;
    // `last_80 + 1..` はすべて 0x00 であるべき (厳格化で真偽判定)。
    if padded[last_80 + 1..].iter().any(|&b| b != 0x00) {
        return None;
    }
    Some(&padded[..last_80])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_selection_boundary() {
        // 1024 - 16 = 1008 byte までは 1K バケット。
        assert_eq!(select_padding_bucket(1008), Some(1024));
        assert_eq!(select_padding_bucket(1009), Some(4096));
        // 262144 - 16 = 262128 までは 256K バケット。
        assert_eq!(select_padding_bucket(262_128), Some(262_144));
        assert_eq!(select_padding_bucket(262_129), None);
    }

    #[test]
    fn pad_then_strip_roundtrip() {
        for len in [0usize, 1, 100, 1007] {
            let input = vec![0xaa; len];
            let padded = pad_iso7816_4(&input, 1024).expect("input < target");
            assert_eq!(padded.len(), 1024);
            assert_eq!(strip_iso7816_4(&padded), Some(input.as_slice()));
        }
    }

    #[test]
    fn pad_returns_none_when_input_too_large() {
        // input == target → terminator が入らないので None。
        assert!(pad_iso7816_4(&[0u8; 32], 32).is_none());
        // input > target も None。
        assert!(pad_iso7816_4(&[0u8; 64], 32).is_none());
    }

    #[test]
    fn strip_rejects_garbage_after_80() {
        let mut padded = pad_iso7816_4(&[1, 2, 3], 32).expect("input < target");
        // 末尾の 0x00 の一つを非ゼロに変えて不正データ化。
        *padded.last_mut().unwrap() = 0x01;
        assert_eq!(strip_iso7816_4(&padded), None);
    }

    #[test]
    fn strip_empty_or_no_80() {
        assert_eq!(strip_iso7816_4(&[]), None);
        assert_eq!(strip_iso7816_4(&[0x00, 0x00]), None);
    }
}
