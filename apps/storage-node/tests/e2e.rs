//! End-to-End Tests for Anarchy Storage Node
//!
//! These tests verify the complete workflow of the storage system
//! including chain interaction and P2P fragment transfer.
//!
//! # Prerequisites
//!
//! These tests require a running Anarchy blockchain node:
//!
//! ```bash
//! # Terminal 1: Start local dev node
//! cd apps/blockchain
//! cargo run --release -- --dev
//!
//! # Terminal 2: Run E2E tests (tests are ignored by default)
//! cd apps/storage-node
//! cargo test --test e2e -- --ignored
//! ```
//!
//! # Environment Variables
//!
//! - `ANARCHY_NODE_URL`: WebSocket URL of the blockchain node (default: `ws://127.0.0.1:9944`)
//!
//! # Test Scenarios
//!
//! - **T068 (fragment_lifecycle)**: Complete fragment lifecycle from registration to holding declaration
//! - **T069 (fragment_retrieval)**: Fragment GET request and response
//! - **T070 (multi_node_transfer)**: Two-node fragment transfer scenario

#[path = "e2e/fragment_lifecycle.rs"]
mod fragment_lifecycle;

#[path = "e2e/fragment_retrieval.rs"]
mod fragment_retrieval;

#[path = "e2e/multi_node_transfer.rs"]
mod multi_node_transfer;
