//! MerkleTree Module
//!
//! Blake2bベースのマークルツリー構築・検証機能を提供。
//!
//! # ドメイン分離 (second-preimage 攻撃対策)
//!
//! リーフと内部ノードでハッシュ入力にプレフィックスを付与し、
//! 「内部ノードの `left || right` (64バイト) を偽リーフとして提示すると
//! 同一ルートに対して検証が通る」二次原像攻撃 (proof forgery) を防ぐ。
//!
//! - リーフ:       `blake2b256(0x00 || leaf_data)`
//! - 内部ノード:   `blake2b256(0x01 || left || right)`
//!
//! chain-node 側の検証 (`apps/blockchain/node/src/rpc/storage.rs` の
//! `verify_merkle_proof`) と完全に同一のスキームでなければならない。

use blake2::{Blake2b, Digest};
use rs_merkle::{Hasher, MerkleProof, MerkleTree};
use wasm_bindgen::prelude::*;

/// リーフハッシュのドメイン分離プレフィックス
pub const MERKLE_LEAF_PREFIX: u8 = 0x00;
/// 内部ノードハッシュのドメイン分離プレフィックス
pub const MERKLE_NODE_PREFIX: u8 = 0x01;

/// Blake2b-256 Hasher for rs_merkle（ドメイン分離付き）
#[derive(Clone)]
pub struct Blake2bHasher;

impl Blake2bHasher {
    /// リーフハッシュ: `blake2b256(0x00 || data)`
    ///
    /// ツリー構築・検証時のリーフは必ずこの関数でハッシュすること
    /// （`Hasher::hash` は生ハッシュなので使わない）。
    pub fn hash_leaf(data: &[u8]) -> [u8; 32] {
        let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
        hasher.update([MERKLE_LEAF_PREFIX]);
        hasher.update(data);
        hasher.finalize().into()
    }
}

impl Hasher for Blake2bHasher {
    type Hash = [u8; 32];

    /// 生の Blake2b-256（プレフィックスなし）。
    ///
    /// ツリー構築には使用されない（`concat_and_hash` を override しているため）。
    /// 汎用ハッシュ export `blake2b_hash` の実体としてのみ残している。
    fn hash(data: &[u8]) -> Self::Hash {
        let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    /// 内部ノード: `blake2b256(0x01 || left || right)`
    ///
    /// 右ノードが無い場合は left をそのまま伝播する
    /// （rs_merkle デフォルトと同じ propagation 規則）。
    fn concat_and_hash(left: &Self::Hash, right: Option<&Self::Hash>) -> Self::Hash {
        match right {
            Some(right_node) => {
                let mut hasher = Blake2b::<blake2::digest::consts::U32>::new();
                hasher.update([MERKLE_NODE_PREFIX]);
                hasher.update(left);
                hasher.update(right_node);
                hasher.finalize().into()
            }
            None => *left,
        }
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

    // リーフはドメイン分離付きハッシュ (0x00 prefix) を使う
    let leaves: Vec<[u8; 32]> = fragments
        .iter()
        .map(|f| Blake2bHasher::hash_leaf(f))
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

    // リーフはドメイン分離付きハッシュ (0x00 prefix) を使う
    let leaf_hash = Blake2bHasher::hash_leaf(leaf_data);

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

    /// 回帰テスト: 内部ノードの `left || right` を偽リーフとして提示する
    /// second-preimage 攻撃 (proof forgery) が失敗することを確認する。
    ///
    /// ドメイン分離前は `hash(h0 || h1)` が内部ノード n01 と一致したため、
    /// 「leaf_index=0, total_leaves=2, proof=[n23]」で root に対して検証が通った。
    #[test]
    fn test_forged_internal_node_leaf_rejected() {
        let fragments: Vec<&[u8]> = vec![
            b"fragment0",
            b"fragment1",
            b"fragment2",
            b"fragment3",
        ];

        let result = merkle_build_internal(&fragments).expect("Build should succeed");

        // リーフハッシュと右側の内部ノード n23 を手計算
        let h0 = Blake2bHasher::hash_leaf(b"fragment0");
        let h1 = Blake2bHasher::hash_leaf(b"fragment1");
        let h2 = Blake2bHasher::hash_leaf(b"fragment2");
        let h3 = Blake2bHasher::hash_leaf(b"fragment3");
        let n23 = Blake2bHasher::concat_and_hash(&h2, Some(&h3));

        // 偽リーフ: 内部ノード n01 の子の連結 (64バイト)
        let mut fake_leaf = Vec::with_capacity(64);
        fake_leaf.extend_from_slice(&h0);
        fake_leaf.extend_from_slice(&h1);

        // 偽Proof: 「リーフ2枚のツリーで index 0、兄弟は n23」と主張
        let fake_proof_bytes: Vec<u8> = n23.to_vec();

        let verified =
            merkle_verify_internal(&result.root, &fake_proof_bytes, &fake_leaf, 0, 2)
                .expect("Verification should not error");
        assert!(
            !verified,
            "Forged internal-node leaf must NOT verify (domain separation)"
        );

        // 健全性確認: ドメイン分離により リーフハッシュ != 内部ノードハッシュ
        let n01 = Blake2bHasher::concat_and_hash(&h0, Some(&h1));
        assert_ne!(
            Blake2bHasher::hash_leaf(&fake_leaf),
            n01,
            "Leaf hash of concatenated children must differ from internal node hash"
        );

        // 正常な Proof は引き続き検証可能であること
        let proof_bytes = result.generate_proof(0).expect("Proof generation should succeed");
        let verified_ok =
            merkle_verify_internal(&result.root, &proof_bytes, b"fragment0", 0, 4)
                .expect("Verification should not error");
        assert!(verified_ok, "Legitimate proof must still verify");
    }

    /// Known-answer test: chain-node 側 (`apps/blockchain/node/src/rpc/storage.rs`)
    /// の同名テストと同じ期待値を共有し、両実装のスキーム一致を担保する。
    /// この値を変更する場合は必ず両方を同時に更新すること。
    #[test]
    fn test_domain_separated_root_known_answer() {
        let fragments: Vec<&[u8]> = vec![
            b"fragment0",
            b"fragment1",
            b"fragment2",
            b"fragment3",
            b"fragment4",
        ];

        let result = merkle_build_internal(&fragments).expect("Build should succeed");
        let root_hex: String = result.root.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(root_hex, EXPECTED_ROOT_HEX_5_FRAGMENTS);
    }

    /// `blake2b256(0x00 || leaf)` / `blake2b256(0x01 || left || right)` スキームでの
    /// fragment0..fragment4 のルート期待値（chain-node 側テストと共有）
    const EXPECTED_ROOT_HEX_5_FRAGMENTS: &str =
        "4d1b2e22c3ad48ee534eff8319420a8be41e389e7c430d6d3149b4338eb9419b";
}
