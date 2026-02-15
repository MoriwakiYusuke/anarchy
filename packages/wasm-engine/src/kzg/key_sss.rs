//! Key SSS Module
//!
//! 32バイト暗号鍵のSSS分割・復元を提供。
//! ハイブリッドアプローチの第3段階として、AES-256鍵をk-of-n分割する。
//!
//! 既存のsharks実装（sss.rs）を再利用。

use crate::sss::{sss_recover_internal, sss_split_internal};

use super::encryption::KEY_SIZE;

/// 鍵SSS エラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySssError {
    /// 無効な鍵サイズ（32バイトでない）
    InvalidKeySize,
    /// 無効な閾値
    InvalidThreshold,
    /// シェア数不足
    InsufficientShares,
    /// 復元失敗
    RecoveryFailed,
    /// 内部エラー
    InternalError(String),
}

impl core::fmt::Display for KeySssError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            KeySssError::InvalidKeySize => {
                write!(f, "Invalid key size: expected {} bytes", KEY_SIZE)
            }
            KeySssError::InvalidThreshold => {
                write!(f, "Invalid threshold: k must be >= 2 and <= n")
            }
            KeySssError::InsufficientShares => write!(f, "Insufficient shares for key recovery"),
            KeySssError::RecoveryFailed => write!(f, "Key recovery failed"),
            KeySssError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

/// 鍵シェア
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyShare {
    /// シェアのインデックス (0-based, 内部でsharks用に+1)
    pub index: u8,
    /// シリアライズされたsharksシェア
    pub data: Vec<u8>,
}

/// 鍵SSS分割結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySplitResult {
    /// n個のシェア
    pub shares: Vec<KeyShare>,
}

/// AES-256鍵をk-of-n SSS分割
///
/// # Arguments
/// * `key` - 32バイトのAES-256鍵
/// * `k` - 復元に必要な最小シェア数
/// * `n` - 総シェア数
///
/// # Returns
/// n個の鍵シェア。任意のk個があれば鍵を復元可能。
pub fn key_split(key: &[u8], k: u8, n: u8) -> Result<KeySplitResult, KeySssError> {
    // バリデーション
    if key.len() != KEY_SIZE {
        return Err(KeySssError::InvalidKeySize);
    }
    if k < 2 || k > n {
        return Err(KeySssError::InvalidThreshold);
    }

    // 既存のSSS実装を使用
    let fragments = sss_split_internal(key, k, n)
        .map_err(|e| KeySssError::InternalError(e))?;

    // シェアを構築
    let shares = fragments
        .into_iter()
        .enumerate()
        .map(|(i, data)| KeyShare {
            index: i as u8,
            data,
        })
        .collect();

    Ok(KeySplitResult { shares })
}

/// k個以上の鍵シェアから元の鍵を復元
///
/// # Arguments
/// * `shares` - 利用可能なシェア
/// * `k` - 最小シェア数
///
/// # Returns
/// 復元された32バイト鍵
pub fn key_recover(shares: &[KeyShare], k: u8) -> Result<[u8; KEY_SIZE], KeySssError> {
    if shares.len() < k as usize {
        return Err(KeySssError::InsufficientShares);
    }

    // シェアデータを抽出
    let fragments: Vec<Vec<u8>> = shares.iter().map(|s| s.data.clone()).collect();

    // 既存のSSS実装で復元
    let recovered = sss_recover_internal(&fragments, k)
        .map_err(|e| KeySssError::InternalError(e))?;

    // 鍵サイズを検証
    if recovered.len() != KEY_SIZE {
        return Err(KeySssError::RecoveryFailed);
    }

    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&recovered);

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kzg::encryption::generate_key;

    #[test]
    fn test_key_split_recover_roundtrip() {
        let key = generate_key().unwrap();
        let k = 3;
        let n = 5;

        let result = key_split(&key, k, n).unwrap();
        assert_eq!(result.shares.len(), n as usize);

        // 全シェアで復元
        let recovered = key_recover(&result.shares, k).unwrap();
        assert_eq!(recovered, key);
    }

    #[test]
    fn test_key_recover_with_minimum_shares() {
        let key = generate_key().unwrap();
        let k = 3;
        let n = 5;

        let result = key_split(&key, k, n).unwrap();

        // k個のシェアのみで復元
        let subset: Vec<KeyShare> = result.shares[0..k as usize].to_vec();
        let recovered = key_recover(&subset, k).unwrap();
        assert_eq!(recovered, key);
    }

    #[test]
    fn test_key_recover_with_any_k_shares() {
        let key = generate_key().unwrap();
        let k = 2;
        let n = 4;

        let result = key_split(&key, k, n).unwrap();

        // シェア0と3のみで復元
        let subset = vec![
            result.shares[0].clone(),
            result.shares[3].clone(),
        ];
        let recovered = key_recover(&subset, k).unwrap();
        assert_eq!(recovered, key);
    }

    #[test]
    fn test_insufficient_shares() {
        let key = generate_key().unwrap();
        let k = 3;
        let n = 5;

        let result = key_split(&key, k, n).unwrap();

        // k-1個のシェアでは復元不可
        let subset: Vec<KeyShare> = result.shares[0..(k - 1) as usize].to_vec();
        let err = key_recover(&subset, k);
        assert_eq!(err, Err(KeySssError::InsufficientShares));
    }

    #[test]
    fn test_invalid_key_size() {
        let short_key = [0u8; 16];
        let err = key_split(&short_key, 2, 3);
        assert_eq!(err, Err(KeySssError::InvalidKeySize));
    }

    #[test]
    fn test_invalid_threshold() {
        let key = generate_key().unwrap();

        // k = 1 は不可（SSSの意味がない）
        assert_eq!(key_split(&key, 1, 3), Err(KeySssError::InvalidThreshold));
        // k > n
        assert_eq!(key_split(&key, 5, 3), Err(KeySssError::InvalidThreshold));
    }

    #[test]
    fn test_2_of_3() {
        let key = generate_key().unwrap();
        let k = 2;
        let n = 3;

        let result = key_split(&key, k, n).unwrap();

        // 各ペアで復元可能か確認
        let pairs = [(0, 1), (0, 2), (1, 2)];
        for (a, b) in pairs {
            let subset = vec![
                result.shares[a].clone(),
                result.shares[b].clone(),
            ];
            let recovered = key_recover(&subset, k).unwrap();
            assert_eq!(recovered, key, "Failed with shares {} and {}", a, b);
        }
    }
}
