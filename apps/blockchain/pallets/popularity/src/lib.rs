//! # Popularity Pallet
//!
//! 投稿人気度スコア管理。reaction による加点と時間減衰、
//! 閾値割れの mark + 猶予期間後の削除を担当する。
//! 詳細: docs/superpowers/specs/2026-05-03-post-popularity-design.md

#![cfg_attr(not(feature = "std"), no_std)]

pub mod decay;

pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Placeholder, // replaced in later tasks
    }

    #[pallet::error]
    pub enum Error<T> {
        Placeholder, // replaced in later tasks
    }
}
