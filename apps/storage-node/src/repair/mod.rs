//! Self-repair module for distributed storage (013-slashing-repair T036)
//!
//! This module implements the fragment repair protocol that enables
//! the storage network to automatically recover from node failures.
//!
//! ## Architecture
//!
//! The repair system uses the following components:
//!
//! - **Protocol**: Message types for P2P repair communication
//! - **Discovery**: Queries chain for AtRisk fragments needing repair
//! - **Coordinator**: Orchestrates share collection and regeneration
//! - **Donor**: Provides shares to coordination requests
//! - **Receiver**: Accepts and stores regenerated shares
//!
//! ## Repair Flow
//!
//! 1. Discovery service polls chain for AtRisk fragments (holder_count <= 4)
//! 2. Coordinator collects k shares from existing holders via CollectShare
//! 3. Coordinator regenerates new share using Lagrange interpolation
//! 4. Coordinator stores new share locally (becoming a holder)
//! 5. Coordinator submits confirm_repair extrinsic to chain
//! 6. Chain updates fragment state (may return to Active if holder_count >= 5)
//!
//! ## KZG-VSS Integration
//!
//! The repair protocol uses the KZG-VSS scheme from wasm-engine:
//! - k-of-n threshold: default 3-of-5
//! - Lagrange interpolation for share regeneration
//! - KZG proofs for on-chain verification

pub mod coordinator;
pub mod discovery;
pub mod donor;
pub mod protocol;
pub mod receiver;
pub mod reporter;

// Re-exports for convenient access
pub use coordinator::{Coordinator, CoordinatorConfig, CoordinatorError, HolderInfo};
pub use discovery::{AtRiskFragment, DiscoveryConfig, DiscoveryError, DiscoveryService};
pub use donor::DonorHandler;
pub use protocol::{
    ContentHash, KzgCommitment, KzgProof, RepairRequest, RepairResponse, ShareData,
    ShareDenialReason, ShareRejectionReason, REPAIR_PROTOCOL_ID,
};
pub use receiver::ReceiverHandler;
pub use reporter::{RepairReporter, ReporterConfig};
