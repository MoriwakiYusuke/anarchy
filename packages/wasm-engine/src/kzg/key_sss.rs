//! Key SSS Module
//!
//! 32バイト暗号鍵のSSS分割・復元を提供。
//! ハイブリッドアプローチの第3段階として、AES-256鍵をk-of-n分割する。
//!
//! GF(256)上のShamirの秘密分散を自前実装（sharksクレート不要）。

use super::encryption::KEY_SIZE;

// =============================================================================
// GF(256) Arithmetic (AES/Rijndael field: x^8 + x^4 + x^3 + x + 1)
// =============================================================================

/// GF(256)上の乗算
fn gf256_mul(a: u8, b: u8) -> u8 {
    let mut result: u8 = 0;
    let mut a = a;
    let mut b = b;

    while b > 0 {
        if b & 1 != 0 {
            result ^= a;
        }
        let hi_bit = a & 0x80;
        a <<= 1;
        if hi_bit != 0 {
            a ^= 0x1b; // x^8 → x^4 + x^3 + x + 1
        }
        b >>= 1;
    }
    result
}

/// GF(256)上の逆元（拡張ユークリッド法代わりにフェルマーの小定理: a^254 = a^(-1)）
fn gf256_inv(a: u8) -> u8 {
    if a == 0 {
        return 0; // 数学的には未定義だが0を返す
    }
    // a^254 = a^(-1) in GF(256)
    let mut result = a;
    for _ in 0..6 {
        result = gf256_mul(result, result);
        result = gf256_mul(result, a);
    }
    gf256_mul(result, result) // 最後に2乗
}

/// GF(256)上の除算: a / b
fn gf256_div(a: u8, b: u8) -> u8 {
    if b == 0 {
        return 0; // エラーケース
    }
    gf256_mul(a, gf256_inv(b))
}

// =============================================================================
// Shamir's Secret Sharing over GF(256)
// =============================================================================

/// 多項式をx点で評価（ホーナー法）
fn poly_eval(coeffs: &[u8], x: u8) -> u8 {
    // coeffs[0] = 秘密, coeffs[1..] = ランダム係数
    let mut result = 0u8;
    for i in (0..coeffs.len()).rev() {
        result = gf256_mul(result, x);
        result ^= coeffs[i]; // GF(256)での加算はXOR
    }
    result
}

/// 単一バイトをk-of-nでSSS分割
fn sss_split_byte(secret: u8, k: u8, n: u8) -> Result<Vec<(u8, u8)>, KeySssError> {
    // k-1個のランダム係数を生成（秘密が定数項）
    let mut coeffs = vec![secret];
    let mut random_bytes = vec![0u8; (k - 1) as usize];
    getrandom::getrandom(&mut random_bytes).map_err(|_| KeySssError::RngFailed)?;
    coeffs.extend(random_bytes);

    // x=1,2,...,n で評価
    Ok((1..=n)
        .map(|x| (x, poly_eval(&coeffs, x)))
        .collect())
}

/// ラグランジュ補間でx=0の値を復元
fn lagrange_interpolate_at_zero(shares: &[(u8, u8)]) -> u8 {
    let mut result = 0u8;

    for (i, &(xi, yi)) in shares.iter().enumerate() {
        // ラグランジュ基底多項式のx=0での値を計算
        let mut num = 1u8;
        let mut den = 1u8;

        for (j, &(xj, _)) in shares.iter().enumerate() {
            if i != j {
                num = gf256_mul(num, xj); // (0 - xj) = xj in GF(256)
                den = gf256_mul(den, xi ^ xj); // (xi - xj)
            }
        }

        let term = gf256_mul(yi, gf256_div(num, den));
        result ^= term;
    }

    result
}

/// 鍵SSS エラー
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySssError {
    /// 乱数生成失敗
    RngFailed,
    /// 無効な鍵サイズ（32バイトでない）
    InvalidKeySize,
    /// 無効な閾値
    InvalidThreshold,
    /// シェア数不足
    InsufficientShares,
    /// 復元失敗
    RecoveryFailed,
    /// 重複または無効なシェアインデックス
    InvalidShareIndex,
}

impl core::fmt::Display for KeySssError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            KeySssError::RngFailed => write!(f, "Random number generation failed"),
            KeySssError::InvalidKeySize => {
                write!(f, "Invalid key size: expected {} bytes", KEY_SIZE)
            }
            KeySssError::InvalidThreshold => {
                write!(f, "Invalid threshold: k must be >= 2 and <= n <= 20")
            }
            KeySssError::InsufficientShares => write!(f, "Insufficient shares for key recovery"),
            KeySssError::RecoveryFailed => write!(f, "Key recovery failed"),
            KeySssError::InvalidShareIndex => write!(f, "Duplicate or zero share index"),
        }
    }
}

/// 鍵シェア
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyShare {
    /// シェアのインデックス (1-based x座標)
    pub index: u8,
    /// シェアデータ（32バイト: 各バイトのy値）
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
    if k < 2 || k > n || n > 20 {
        return Err(KeySssError::InvalidThreshold);
    }

    // 各バイトを独立にSSS分割
    let mut shares: Vec<KeyShare> = (1..=n)
        .map(|x| KeyShare {
            index: x,
            data: Vec::with_capacity(KEY_SIZE),
        })
        .collect();

    for &byte in key {
        let byte_shares = sss_split_byte(byte, k, n)?;
        for (i, (_, y)) in byte_shares.into_iter().enumerate() {
            shares[i].data.push(y);
        }
    }

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

    // シェアサイズ確認
    if !shares.iter().all(|s| s.data.len() == KEY_SIZE) {
        return Err(KeySssError::RecoveryFailed);
    }

    // インデックス検証: 重複・ゼロチェック
    let used_shares = &shares[..k as usize];
    for share in used_shares.iter() {
        if share.index == 0 {
            return Err(KeySssError::InvalidShareIndex);
        }
    }
    let mut seen_indices = [false; 256];
    for share in used_shares.iter() {
        if seen_indices[share.index as usize] {
            return Err(KeySssError::InvalidShareIndex);
        }
        seen_indices[share.index as usize] = true;
    }

    // 各バイト位置で補間
    let mut key = [0u8; KEY_SIZE];
    for i in 0..KEY_SIZE {
        let points: Vec<(u8, u8)> = shares
            .iter()
            .take(k as usize)
            .map(|s| (s.index, s.data[i]))
            .collect();
        key[i] = lagrange_interpolate_at_zero(&points);
    }

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

        let result = key_split(&*key, k, n).unwrap();
        assert_eq!(result.shares.len(), n as usize);

        // 全シェアで復元
        let recovered = key_recover(&result.shares, k).unwrap();
        assert_eq!(recovered, *key);
    }

    #[test]
    fn test_key_recover_with_minimum_shares() {
        let key = generate_key().unwrap();
        let k = 3;
        let n = 5;

        let result = key_split(&*key, k, n).unwrap();

        // k個のシェアのみで復元
        let subset: Vec<KeyShare> = result.shares[0..k as usize].to_vec();
        let recovered = key_recover(&subset, k).unwrap();
        assert_eq!(recovered, *key);
    }

    #[test]
    fn test_key_recover_with_any_k_shares() {
        let key = generate_key().unwrap();
        let k = 2;
        let n = 4;

        let result = key_split(&*key, k, n).unwrap();

        // シェア0と3のみで復元
        let subset = vec![
            result.shares[0].clone(),
            result.shares[3].clone(),
        ];
        let recovered = key_recover(&subset, k).unwrap();
        assert_eq!(recovered, *key);
    }

    #[test]
    fn test_insufficient_shares() {
        let key = generate_key().unwrap();
        let k = 3;
        let n = 5;

        let result = key_split(&*key, k, n).unwrap();

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
        assert_eq!(key_split(&*key, 1, 3), Err(KeySssError::InvalidThreshold));
        // k > n
        assert_eq!(key_split(&*key, 5, 3), Err(KeySssError::InvalidThreshold));
    }

    #[test]
    fn test_2_of_3() {
        let key = generate_key().unwrap();
        let k = 2;
        let n = 3;

        let result = key_split(&*key, k, n).unwrap();

        // 各ペアで復元可能か確認
        let pairs = [(0, 1), (0, 2), (1, 2)];
        for (a, b) in pairs {
            let subset = vec![
                result.shares[a].clone(),
                result.shares[b].clone(),
            ];
            let recovered = key_recover(&subset, k).unwrap();
            assert_eq!(recovered, *key, "Failed with shares {} and {}", a, b);
        }
    }

    #[test]
    fn test_duplicate_index_rejected() {
        let key = generate_key().unwrap();
        let result = key_split(&*key, 3, 5).unwrap();

        // 同じシェアを重複して渡す（重複インデックス）
        let duplicate_shares = vec![
            result.shares[0].clone(),
            result.shares[0].clone(),  // duplicate
            result.shares[1].clone(),
        ];
        assert_eq!(
            key_recover(&duplicate_shares, 3),
            Err(KeySssError::InvalidShareIndex)
        );
    }

    #[test]
    fn test_zero_index_rejected() {
        // 手動でインデックス0のシェアを作成
        let zero_share = KeyShare {
            index: 0,
            data: vec![0u8; KEY_SIZE],
        };
        let valid_share = KeyShare {
            index: 1,
            data: vec![1u8; KEY_SIZE],
        };
        let shares = vec![zero_share, valid_share.clone(), valid_share];
        
        assert_eq!(
            key_recover(&shares, 2),
            Err(KeySssError::InvalidShareIndex)
        );
    }
}
