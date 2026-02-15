//! Reed-Solomon Erasure Coding Module
//!
//! k-of-n erasure codingを提供。
//! ハイブリッドアプローチの第2段階として、暗号文をk-of-nチャンクに分割する。

use reed_solomon_erasure::galois_8::ReedSolomon;

/// Reed-Solomonエラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RsError {
    /// 無効な閾値 (k > n or k < 1 or n < 2)
    InvalidThreshold,
    /// データが空
    EmptyData,
    /// シェア数不足 (k個未満の有効シェア)
    InsufficientShards,
    /// シェアサイズ不一致
    ShardSizeMismatch,
    /// エンコード失敗
    EncodeFailed,
    /// デコード失敗
    DecodeFailed,
    /// 無効なシェアインデックス
    InvalidShardIndex,
}

impl core::fmt::Display for RsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RsError::InvalidThreshold => write!(f, "Invalid threshold: k must be >= 1 and <= n, n >= 2"),
            RsError::EmptyData => write!(f, "Data cannot be empty"),
            RsError::InsufficientShards => write!(f, "Insufficient shards for recovery"),
            RsError::ShardSizeMismatch => write!(f, "Shard sizes do not match"),
            RsError::EncodeFailed => write!(f, "Reed-Solomon encoding failed"),
            RsError::DecodeFailed => write!(f, "Reed-Solomon decoding failed"),
            RsError::InvalidShardIndex => write!(f, "Invalid shard index"),
        }
    }
}

/// Reed-Solomonシェア
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsShard {
    /// シェアのインデックス (0..n)
    pub index: usize,
    /// シェアのデータ
    pub data: Vec<u8>,
}

/// Reed-Solomonエンコード結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsEncodeResult {
    /// n個のシェア（k個のデータシェア + (n-k)個のパリティシェア）
    pub shards: Vec<RsShard>,
    /// 元データの長さ（パディング除去用）
    pub original_len: usize,
    /// シェアあたりのサイズ
    pub shard_size: usize,
}

/// データをk-of-n Reed-Solomonシェアにエンコード
///
/// # Arguments
/// * `data` - エンコードするデータ
/// * `k` - 復元に必要な最小シェア数（データシェア数）
/// * `n` - 総シェア数
///
/// # Returns
/// n個のシェア。任意のk個があれば元データを復元可能。
pub fn rs_encode(data: &[u8], k: usize, n: usize) -> Result<RsEncodeResult, RsError> {
    // バリデーション
    if k < 1 || k > n || n < 2 {
        return Err(RsError::InvalidThreshold);
    }
    if data.is_empty() {
        return Err(RsError::EmptyData);
    }

    let parity_count = n - k;
    let rs = ReedSolomon::new(k, parity_count).map_err(|_| RsError::InvalidThreshold)?;

    // シェアサイズを計算（データを k 等分、切り上げ）
    let shard_size = (data.len() + k - 1) / k;
    let original_len = data.len();

    // データシェアを作成（パディングあり）
    let mut shards: Vec<Vec<u8>> = Vec::with_capacity(n);
    for i in 0..k {
        let start = i * shard_size;
        let end = core::cmp::min(start + shard_size, data.len());
        let mut shard = Vec::with_capacity(shard_size);
        if start < data.len() {
            shard.extend_from_slice(&data[start..end]);
        }
        // パディング
        shard.resize(shard_size, 0);
        shards.push(shard);
    }

    // パリティシェアを追加
    for _ in 0..parity_count {
        shards.push(vec![0u8; shard_size]);
    }

    // エンコード（パリティを計算）
    rs.encode(&mut shards).map_err(|_| RsError::EncodeFailed)?;

    // 結果を構築
    let result_shards = shards
        .into_iter()
        .enumerate()
        .map(|(index, data)| RsShard { index, data })
        .collect();

    Ok(RsEncodeResult {
        shards: result_shards,
        original_len,
        shard_size,
    })
}

/// k個以上のシェアから元データを復元
///
/// # Arguments
/// * `shards` - 利用可能なシェア（Option<RsShard>のスライス、欠損はNone）
/// * `k` - 最小シェア数
/// * `n` - 総シェア数
/// * `original_len` - 元データの長さ
/// * `shard_size` - シェアサイズ
///
/// # Returns
/// 復元されたデータ
pub fn rs_decode(
    shards: &[Option<RsShard>],
    k: usize,
    n: usize,
    original_len: usize,
    shard_size: usize,
) -> Result<Vec<u8>, RsError> {
    // バリデーション
    if k < 1 || k > n || n < 2 {
        return Err(RsError::InvalidThreshold);
    }
    if shards.len() != n {
        return Err(RsError::InsufficientShards);
    }

    // 有効シェア数をカウント
    let valid_count = shards.iter().filter(|s| s.is_some()).count();
    if valid_count < k {
        return Err(RsError::InsufficientShards);
    }

    let parity_count = n - k;
    let rs = ReedSolomon::new(k, parity_count).map_err(|_| RsError::InvalidThreshold)?;

    // シェア配列を構築
    let mut shard_data: Vec<Option<Vec<u8>>> = shards
        .iter()
        .map(|opt| opt.as_ref().map(|s| s.data.clone()))
        .collect();

    // サイズ検証
    for shard_opt in &shard_data {
        if let Some(shard) = shard_opt {
            if shard.len() != shard_size {
                return Err(RsError::ShardSizeMismatch);
            }
        }
    }

    // 復元
    rs.reconstruct(&mut shard_data).map_err(|_| RsError::DecodeFailed)?;

    // データシェアを結合
    let mut result = Vec::with_capacity(k * shard_size);
    for shard_opt in shard_data.iter().take(k) {
        if let Some(shard) = shard_opt {
            result.extend_from_slice(shard);
        } else {
            return Err(RsError::DecodeFailed);
        }
    }

    // パディングを除去
    result.truncate(original_len);

    Ok(result)
}

/// シェアリストから復元（便利関数）
///
/// インデックス付きシェアのリストからOption配列を構築して復元
pub fn rs_decode_from_shards(
    available_shards: &[RsShard],
    k: usize,
    n: usize,
    original_len: usize,
    shard_size: usize,
) -> Result<Vec<u8>, RsError> {
    if available_shards.len() < k {
        return Err(RsError::InsufficientShards);
    }

    // Option配列を構築
    let mut shards: Vec<Option<RsShard>> = vec![None; n];
    for shard in available_shards {
        if shard.index >= n {
            return Err(RsError::InvalidShardIndex);
        }
        shards[shard.index] = Some(shard.clone());
    }

    rs_decode(&shards, k, n, original_len, shard_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let data = b"Hello, Anarchy! This is a test message.";
        let k = 3;
        let n = 5;

        let encoded = rs_encode(data, k, n).unwrap();
        assert_eq!(encoded.shards.len(), n);
        assert_eq!(encoded.original_len, data.len());

        // 全シェアから復元
        let shards: Vec<Option<RsShard>> = encoded.shards.iter().cloned().map(Some).collect();
        let decoded = rs_decode(&shards, k, n, encoded.original_len, encoded.shard_size).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decode_with_minimum_shards() {
        let data = b"Secret data that needs protection";
        let k = 3;
        let n = 5;

        let encoded = rs_encode(data, k, n).unwrap();

        // k個のシェアのみで復元（最初のk個）
        let mut shards: Vec<Option<RsShard>> = vec![None; n];
        for i in 0..k {
            shards[i] = Some(encoded.shards[i].clone());
        }

        let decoded = rs_decode(&shards, k, n, encoded.original_len, encoded.shard_size).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decode_with_any_k_shards() {
        let data = b"Test data for any-k recovery";
        let k = 2;
        let n = 4;

        let encoded = rs_encode(data, k, n).unwrap();

        // シェア0と3のみ（パリティシェアを含む）
        let mut shards: Vec<Option<RsShard>> = vec![None; n];
        shards[0] = Some(encoded.shards[0].clone());
        shards[3] = Some(encoded.shards[3].clone());

        let decoded = rs_decode(&shards, k, n, encoded.original_len, encoded.shard_size).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decode_from_shards_helper() {
        let data = b"Using helper function";
        let k = 2;
        let n = 3;

        let encoded = rs_encode(data, k, n).unwrap();

        // インデックス1と2のシェアのみ
        let available = vec![
            encoded.shards[1].clone(),
            encoded.shards[2].clone(),
        ];

        let decoded = rs_decode_from_shards(&available, k, n, encoded.original_len, encoded.shard_size).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_insufficient_shards() {
        let data = b"Not enough shards";
        let k = 3;
        let n = 5;

        let encoded = rs_encode(data, k, n).unwrap();

        // k-1個のシェアでは復元不可
        let mut shards: Vec<Option<RsShard>> = vec![None; n];
        for i in 0..(k - 1) {
            shards[i] = Some(encoded.shards[i].clone());
        }

        let result = rs_decode(&shards, k, n, encoded.original_len, encoded.shard_size);
        assert_eq!(result, Err(RsError::InsufficientShards));
    }

    #[test]
    fn test_invalid_threshold() {
        let data = b"test";

        // k > n
        assert_eq!(rs_encode(data, 5, 3), Err(RsError::InvalidThreshold));
        // k = 0
        assert_eq!(rs_encode(data, 0, 3), Err(RsError::InvalidThreshold));
        // n = 1
        assert_eq!(rs_encode(data, 1, 1), Err(RsError::InvalidThreshold));
    }

    #[test]
    fn test_empty_data() {
        assert_eq!(rs_encode(b"", 2, 3), Err(RsError::EmptyData));
    }

    #[test]
    fn test_large_data() {
        // 100KBのデータ
        let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let k = 5;
        let n = 10;

        let encoded = rs_encode(&data, k, n).unwrap();

        // シェア 0, 2, 4, 6, 8 のみで復元
        let mut shards: Vec<Option<RsShard>> = vec![None; n];
        for i in (0..n).step_by(2) {
            shards[i] = Some(encoded.shards[i].clone());
        }

        let decoded = rs_decode(&shards, k, n, encoded.original_len, encoded.shard_size).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_2_of_3() {
        let data = b"Simple 2-of-3 test";
        let k = 2;
        let n = 3;

        let encoded = rs_encode(data, k, n).unwrap();

        // 各ペアで復元可能か確認
        let pairs = [(0, 1), (0, 2), (1, 2)];
        for (a, b) in pairs {
            let mut shards: Vec<Option<RsShard>> = vec![None; n];
            shards[a] = Some(encoded.shards[a].clone());
            shards[b] = Some(encoded.shards[b].clone());

            let decoded = rs_decode(&shards, k, n, encoded.original_len, encoded.shard_size).unwrap();
            assert_eq!(decoded, data.to_vec(), "Failed with shards {} and {}", a, b);
        }
    }
}
