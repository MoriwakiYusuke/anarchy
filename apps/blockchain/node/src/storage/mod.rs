//! Storage node communication module
//!
//! Provides session management and HTTP client for storage node communication.
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────┐           ┌────────────────────┐
//! │  Blockchain Node   │           │   Storage Node     │
//! └─────────┬──────────┘           └─────────┬──────────┘
//!           │                                │
//!           │ POST /session (signed)         │
//!           │───────────────────────────────>│
//!           │                                │ Verify signature
//!           │ { token, expires_at }          │
//!           │<───────────────────────────────│
//!           │                                │
//!           │ POST /fragments                │
//!           │ X-Session-Token: <token>       │
//!           │───────────────────────────────>│
//!           │                                │ Token validation
//! ```

pub mod session_client;

pub use session_client::StorageSessionClient;
