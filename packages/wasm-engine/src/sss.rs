//! SSS (Shamir's Secret Sharing) Module
//!
//! k-of-n 閾値秘密分散の分割・復元機能を提供。

use sharks::{Share, Sharks};
use wasm_bindgen::prelude::*;

/// SSS分割結果
#[wasm_bindgen]
pub struct SplitResult {
    /// 分割された断片（シリアライズ済み）
    fragments: Vec<Vec<u8>>,
}

#[wasm_bindgen]
impl SplitResult {
    /// 断片数を取得
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.fragments.len()
    }

    /// 指定インデックスの断片を取得
    pub fn get_fragment(&self, index: usize) -> Option<Vec<u8>> {
        self.fragments.get(index).cloned()
    }

    /// 全断片を取得
    pub fn get_all_fragments(&self) -> Vec<js_sys::Uint8Array> {
        self.fragments
            .iter()
            .map(|f| js_sys::Uint8Array::from(f.as_slice()))
            .collect()
    }
}

/// 内部実装: データをk-of-nで分割（テスト可能）
/// Maximum allowed value for n (total fragments)
const MAX_N: u8 = 20;

pub fn sss_split_internal(data: &[u8], k: u8, n: u8) -> Result<Vec<Vec<u8>>, String> {
    if k == 0 || n == 0 || k > n {
        return Err("Invalid k/n parameters: k must be > 0 and <= n".to_string());
    }
    if n > MAX_N {
        return Err(format!("n must be <= {} for practical use", MAX_N));
    }
    if data.is_empty() {
        return Err("Cannot split empty data".to_string());
    }

    let sharks = Sharks(k);
    let dealer = sharks.dealer(data);
    
    let shares: Vec<Share> = dealer.take(n as usize).collect();
    
    let fragments: Vec<Vec<u8>> = shares
        .into_iter()
        .map(|share| Vec::from(&share))
        .collect();

    Ok(fragments)
}

/// 内部実装: k個以上の断片から元データを復元（テスト可能）
pub fn sss_recover_internal(fragments: &[Vec<u8>], k: u8) -> Result<Vec<u8>, String> {
    if fragments.len() < k as usize {
        return Err(format!(
            "Insufficient fragments: got {}, need at least {}",
            fragments.len(),
            k
        ));
    }

    let sharks = Sharks(k);
    
    let shares: Result<Vec<Share>, _> = fragments
        .iter()
        .map(|f| Share::try_from(f.as_slice()))
        .collect();
    
    let shares = shares.map_err(|e| format!("Invalid share format: {:?}", e))?;

    sharks
        .recover(&shares)
        .map_err(|e| format!("Recovery failed: {:?}", e))
}

/// データをk-of-nで分割
///
/// # Arguments
/// * `data` - 分割対象データ
/// * `k` - 復元に必要な最小断片数（しきい値）
/// * `n` - 総断片数
///
/// # Returns
/// * `SplitResult` - n個の断片を含む結果
#[wasm_bindgen]
pub fn sss_split(data: &[u8], k: u8, n: u8) -> Result<SplitResult, JsError> {
    let fragments = sss_split_internal(data, k, n).map_err(|e| JsError::new(&e))?;
    Ok(SplitResult { fragments })
}

/// k個以上の断片から元データを復元
///
/// # Arguments
/// * `fragments` - 断片の配列（シリアライズ済み）
/// * `k` - 復元に必要な最小断片数
///
/// # Returns
/// * `Vec<u8>` - 復元されたデータ
#[wasm_bindgen]
pub fn sss_recover(fragments: Vec<js_sys::Uint8Array>, k: u8) -> Result<Vec<u8>, JsError> {
    let fragment_vecs: Vec<Vec<u8>> = fragments.iter().map(|f| f.to_vec()).collect();
    sss_recover_internal(&fragment_vecs, k).map_err(|e| JsError::new(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_recover() {
        let data = b"Hello, Anarchy!";
        let k = 3;
        let n = 5;

        // 分割
        let fragments = sss_split_internal(data, k, n).expect("Split should succeed");
        assert_eq!(fragments.len(), n as usize);

        // k個の断片で復元
        let subset: Vec<Vec<u8>> = fragments[0..k as usize].to_vec();
        let recovered = sss_recover_internal(&subset, k).expect("Recovery should succeed");
        assert_eq!(recovered, data);
    }

    #[test]
    fn test_insufficient_shares() {
        let data = b"Secret data";
        let k = 3;
        let n = 5;

        let fragments = sss_split_internal(data, k, n).expect("Split should succeed");

        // k-1個の断片では復元できない
        let subset: Vec<Vec<u8>> = fragments[0..(k - 1) as usize].to_vec();
        let result = sss_recover_internal(&subset, k);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_parameters() {
        let data = b"Test";

        // k = 0
        assert!(sss_split_internal(data, 0, 5).is_err());

        // k > n
        assert!(sss_split_internal(data, 6, 5).is_err());

        // n > MAX_N
        assert!(sss_split_internal(data, 3, 21).is_err());

        // empty data
        assert!(sss_split_internal(b"", 2, 3).is_err());
    }
}
