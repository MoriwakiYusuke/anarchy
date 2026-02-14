//! Rate limiting for storage pallet extrinsics
//!
//! Implements per-block rate limiting for:
//! - Node registration (FR-410): max 5 registrations per block
//! - Holding declarations (FR-406): max 10 declarations per block per node
//!
//! Rate limits are enforced using per-block counters that are cleared
//! in the on_finalize hook.

/// Check if a new node registration is allowed in the current block.
///
/// # Arguments
/// * `current_count` - Current number of registrations in this block
/// * `max_per_block` - Maximum allowed registrations per block
///
/// # Returns
/// `true` if registration is allowed, `false` if rate limit exceeded
pub fn can_register_node(current_count: u32, max_per_block: u32) -> bool {
    current_count < max_per_block
}

/// Check if a new holding declaration is allowed for a node in the current block.
///
/// # Arguments
/// * `current_count` - Current number of declarations by this node in this block
/// * `max_per_block_per_node` - Maximum allowed declarations per block per node
///
/// # Returns
/// `true` if declaration is allowed, `false` if rate limit exceeded
pub fn can_declare_holding(current_count: u32, max_per_block_per_node: u32) -> bool {
    current_count < max_per_block_per_node
}

/// Increment the registration counter for the current block.
///
/// # Arguments
/// * `current_count` - Current counter value
///
/// # Returns
/// Incremented counter value
pub fn increment_registration_count(current_count: u32) -> u32 {
    current_count.saturating_add(1)
}

/// Increment the declaration counter for a node in the current block.
///
/// # Arguments
/// * `current_count` - Current counter value
///
/// # Returns
/// Incremented counter value
pub fn increment_declaration_count(current_count: u32) -> u32 {
    current_count.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_register_node() {
        assert!(can_register_node(0, 5));
        assert!(can_register_node(4, 5));
        assert!(!can_register_node(5, 5));
        assert!(!can_register_node(10, 5));
    }

    #[test]
    fn test_can_declare_holding() {
        assert!(can_declare_holding(0, 10));
        assert!(can_declare_holding(9, 10));
        assert!(!can_declare_holding(10, 10));
        assert!(!can_declare_holding(20, 10));
    }

    #[test]
    fn test_increment_counters() {
        assert_eq!(increment_registration_count(0), 1);
        assert_eq!(increment_registration_count(4), 5);
        assert_eq!(increment_declaration_count(0), 1);
        assert_eq!(increment_declaration_count(9), 10);
    }

    #[test]
    fn test_saturating_increment() {
        // Should not overflow
        assert_eq!(increment_registration_count(u32::MAX), u32::MAX);
        assert_eq!(increment_declaration_count(u32::MAX), u32::MAX);
    }
}
