//! MerkleTree Module
//!
//! Blake2bベースのマークルツリー構築・検証機能を提供。

use blake2::{Blake2b, Digest};
use rs_merkle::{Hasher, MerkleProof, MerkleTree};
use wasm_bindgen::prelude::*;

/// Blake2b-256 Hasher for rs_merkle
#[derive(Clone)]
pub struct Blake2bHasher;

impl Hasher for Blake2bHasher {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> Self::Hash {
        let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
        hasher.update(data);
        hasher.finalize().into()
    }
}

/// 内部用MerkleTree結果（テスト可能）
pub struct MerkleResultInternal {
    pub root: [u8; 32],
    pub tree: MerkleTree<Blake2bHasher>,
    pub leaf_count: usize,
}

impl MerkleResultInternal {
    /// 指定インデックスのMerkleProofを生成
    pub fn generate_proof(&self, index: usize) -> Result<Vec<u8>, String> {
        if index >= self.leaf_count {
            return Err(format!(
                "Index {} out of bounds (leaf_count: {})",
                index, self.leaf_count
            ));
        }

        let proof = self.tree.proof(&[index]);
        Ok(proof.to_bytes())
    }
}

/// 内部実装: 断片リストからMerkleTreeを構築（テスト可能）
pub fn merkle_build_internal(fragments: &[&[u8]]) -> Result<MerkleResultInternal, String> {
    if fragments.is_empty() {
        return Err("Cannot build MerkleTree from empty fragments".to_string());
    }

    let leaves: Vec<[u8; 32]> = fragments
        .iter()
        .map(|f| Blake2bHasher::hash(f))
        .collect();

    let tree = MerkleTree::<Blake2bHasher>::from_leaves(&leaves);
    
    let root = tree
        .root()
        .ok_or_else(|| "Failed to compute MerkleRoot".to_string())?;

    Ok(MerkleResultInternal {
        root,
        tree,
        leaf_count: leaves.len(),
    })
}

/// 内部実装: MerkleProofを検証（テスト可能）
pub fn merkle_verify_internal(
    root: &[u8; 32],
    proof_bytes: &[u8],
    leaf_data: &[u8],
    leaf_index: usize,
    total_leaves: usize,
) -> Result<bool, String> {
    if total_leaves == 0 {
        return Err("total_leaves must be > 0".to_string());
    }
    if leaf_index >= total_leaves {
        return Err(format!(
            "leaf_index {} out of bounds (total_leaves: {})", leaf_index, total_leaves
        ));
    }

    let proof = MerkleProof::<Blake2bHasher>::from_bytes(proof_bytes)
        .map_err(|e| format!("Invalid proof format: {:?}", e))?;

    let leaf_hash = Blake2bHasher::hash(leaf_data);

    Ok(proof.verify(*root, &[leaf_index], &[leaf_hash], total_leaves))
}

/// MerkleTree構築結果（Wasm用）
#[wasm_bindgen]
pub struct MerkleResult {
    root: [u8; 32],
    tree: MerkleTree<Blake2bHasher>,
    leaf_count: usize,
}

#[wasm_bindgen]
impl MerkleResult {
    /// MerkleRootを取得
    #[wasm_bindgen(getter)]
    pub fn root(&self) -> Vec<u8> {
        self.root.to_vec()
    }

    /// リーフ数を取得
    #[wasm_bindgen(getter)]
    pub fn leaf_count(&self) -> usize {
        self.leaf_count
    }

    /// 指定インデックスのMerkleProofを生成
    pub fn generate_proof(&self, index: usize) -> Result<Vec<u8>, JsError> {
        if index >= self.leaf_count {
            return Err(JsError::new(&format!(
                "Index {} out of bounds (leaf_count: {})",
                index, self.leaf_count
            )));
        }

        let proof = self.tree.proof(&[index]);
        Ok(proof.to_bytes())
    }
}

/// 断片リストからMerkleTreeを構築
///
/// # Arguments
/// * `fragments` - 断片データの配列（各断片のハッシュがリーフになる）
///
/// # Returns
/// * `MerkleResult` - MerkleRootとProof生成機能を持つ結果
#[wasm_bindgen]
pub fn merkle_build(fragments: Vec<js_sys::Uint8Array>) -> Result<MerkleResult, JsError> {
    if fragments.is_empty() {
        return Err(JsError::new("Cannot build MerkleTree from empty fragments"));
    }

    let fragment_vecs: Vec<Vec<u8>> = fragments.iter().map(|f| f.to_vec()).collect();
    let fragment_slices: Vec<&[u8]> = fragment_vecs.iter().map(|v| v.as_slice()).collect();
    
    let internal = merkle_build_internal(&fragment_slices)
        .map_err(|e| JsError::new(&e))?;

    Ok(MerkleResult {
        root: internal.root,
        tree: internal.tree,
        leaf_count: internal.leaf_count,
    })
}

/// MerkleProofを検証
///
/// # Arguments
/// * `root` - MerkleRoot (32 bytes)
/// * `proof_bytes` - シリアライズされたMerkleProof
/// * `leaf_data` - 検証対象のリーフデータ
/// * `leaf_index` - リーフのインデックス
/// * `total_leaves` - 総リーフ数
///
/// # Returns
/// * `bool` - 検証成功ならtrue
#[wasm_bindgen]
pub fn merkle_verify(
    root: &[u8],
    proof_bytes: &[u8],
    leaf_data: &[u8],
    leaf_index: usize,
    total_leaves: usize,
) -> Result<bool, JsError> {
    if root.len() != 32 {
        return Err(JsError::new("Invalid root length: expected 32 bytes"));
    }
    if total_leaves == 0 {
        return Err(JsError::new("total_leaves must be > 0"));
    }
    if leaf_index >= total_leaves {
        return Err(JsError::new(&format!(
            "leaf_index {} out of bounds (total_leaves: {})", leaf_index, total_leaves
        )));
    }

    let root_array: [u8; 32] = root
        .try_into()
        .map_err(|_| JsError::new("Failed to convert root to [u8; 32]"))?;

    merkle_verify_internal(&root_array, proof_bytes, leaf_data, leaf_index, total_leaves)
        .map_err(|e| JsError::new(&e))
}

/// 単一データのBlake2bハッシュを計算
#[wasm_bindgen]
pub fn blake2b_hash(data: &[u8]) -> Vec<u8> {
    Blake2bHasher::hash(data).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_tree() {
        let fragments: Vec<&[u8]> = vec![
            b"fragment0",
            b"fragment1",
            b"fragment2",
            b"fragment3",
            b"fragment4",
        ];

        let result = merkle_build_internal(&fragments).expect("Build should succeed");
        assert_eq!(result.leaf_count, 5);
        assert_eq!(result.root.len(), 32);
    }

    #[test]
    fn test_proof_verify() {
        let fragment_data: Vec<&[u8]> = vec![
            b"fragment0",
            b"fragment1",
            b"fragment2",
            b"fragment3",
            b"fragment4",
        ];

        let result = merkle_build_internal(&fragment_data).expect("Build should succeed");

        // インデックス2のProofを生成・検証
        let proof_bytes = result.generate_proof(2).expect("Proof generation should succeed");
        
        let verified = merkle_verify_internal(&result.root, &proof_bytes, b"fragment2", 2, 5)
            .expect("Verification should not error");
        
        assert!(verified, "Valid proof should verify");
    }

    #[test]
    fn test_proof_reject_invalid() {
        let fragment_data: Vec<&[u8]> = vec![
            b"fragment0",
            b"fragment1",
            b"fragment2",
        ];

        let result = merkle_build_internal(&fragment_data).expect("Build should succeed");

        let proof_bytes = result.generate_proof(1).expect("Proof generation should succeed");
        
        // 異なるデータで検証 → 失敗すべき
        let verified = merkle_verify_internal(&result.root, &proof_bytes, b"wrong_data", 1, 3)
            .expect("Verification should not error");
        
        assert!(!verified, "Invalid data should not verify");

        // 異なるインデックスで検証 → 失敗すべき
        let verified = merkle_verify_internal(&result.root, &proof_bytes, b"fragment1", 0, 3)
            .expect("Verification should not error");
        
        assert!(!verified, "Wrong index should not verify");
    }

    #[test]
    fn test_blake2b_hash() {
        let data = b"test data";
        let hash = blake2b_hash(data);
        assert_eq!(hash.len(), 32);
        
        // 同じ入力は同じ出力
        let hash2 = blake2b_hash(data);
        assert_eq!(hash, hash2);
    }
}
