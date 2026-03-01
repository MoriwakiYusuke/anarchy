//! Session management for storage node access control
//!
//! Implements session token authentication for blockchain nodes.
//! - P2P connected peers can request session tokens via `storage_requestSession`
//! - HTTP write/delete operations require valid session tokens
//! - Read operations remain unauthenticated
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────┐          ┌─────────────────────┐
//! │  Blockchain Node     │          │  Storage Node       │
//! └──────────┬───────────┘          └─────────┬───────────┘
//!            │                                  │
//!            │ ① storage_requestSession         │
//!            │   (Ed25519 signature)            │
//!            │─────────────────────────────────>│
//!            │                                  │ Verify → Generate token
//!            │ ② token                          │ HashMap<Token, SessionInfo>
//!            │<─────────────────────────────────│
//!            │                                  │
//!            │ ③ HTTP RPC + X-Session-Token     │
//!            │─────────────────────────────────>│
//!            │                                  │ Token validation (fast)
//! ```

mod token;
mod registry;
mod peers;
mod protocol;
mod error;
mod nonce;

pub use token::{SessionToken, SessionInfo};
pub use registry::SessionRegistry;
pub use peers::ConnectedPeers;
pub use protocol::{
    SessionRequest, SessionResponse, SessionProtocolCodec,
    SESSION_PROTOCOL,
};
pub use error::SessionError;
pub use nonce::NonceCache;

#[cfg(test)]
mod tests;
