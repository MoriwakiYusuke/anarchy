//! Blake2b hashing utilities

use blake2::{Blake2b, Digest};
use blake2::digest::consts::U32;

/// Blake2b-256 ハッシュを計算
pub fn blake2b_256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 複数のデータを連結してBlake2b-256ハッシュを計算
pub fn blake2b_256_concat(data: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Blake2b::<U32>::new();
    for d in data {
        hasher.update(d);
    }
    hasher.finalize().into()
}
