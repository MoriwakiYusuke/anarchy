//! Session module integration tests

use super::*;
use libp2p::PeerId;
use std::time::Duration;

#[test]
fn test_full_session_flow() {
    // Create registry
    let registry = SessionRegistry::new();
    let peers = ConnectedPeers::new();

    // Simulate peer connection
    let peer_id = PeerId::random();
    peers.add(peer_id);

    // Verify peer is connected
    assert!(peers.contains(&peer_id));

    // Create session
    let token = registry.create_session(peer_id);

    // Validate session
    let validated = registry.validate(token.as_str());
    assert_eq!(validated, Some(peer_id));

    // Session count should be 1
    assert_eq!(registry.session_count(), 1);

    // Disconnect peer
    peers.remove(&peer_id);
    assert!(!peers.contains(&peer_id));

    // Token should still be valid (session persists after disconnect)
    assert!(registry.validate(token.as_str()).is_some());

    // Revoke session
    registry.revoke_for_peer(&peer_id);
    assert!(registry.validate(token.as_str()).is_none());
    assert_eq!(registry.session_count(), 0);
}

#[test]
fn test_multiple_peers() {
    let registry = SessionRegistry::new();
    let peers = ConnectedPeers::new();

    let peer1 = PeerId::random();
    let peer2 = PeerId::random();
    let peer3 = PeerId::random();

    // Connect all peers
    peers.add(peer1);
    peers.add(peer2);
    peers.add(peer3);

    // Create sessions for all
    let token1 = registry.create_session(peer1);
    let token2 = registry.create_session(peer2);
    let token3 = registry.create_session(peer3);

    assert_eq!(registry.session_count(), 3);
    assert_eq!(peers.count(), 3);

    // All tokens should be valid
    assert_eq!(registry.validate(token1.as_str()), Some(peer1));
    assert_eq!(registry.validate(token2.as_str()), Some(peer2));
    assert_eq!(registry.validate(token3.as_str()), Some(peer3));

    // Revoke one
    registry.revoke_for_peer(&peer2);
    assert_eq!(registry.session_count(), 2);
    assert!(registry.validate(token2.as_str()).is_none());

    // Others still valid
    assert!(registry.validate(token1.as_str()).is_some());
    assert!(registry.validate(token3.as_str()).is_some());
}

#[test]
fn test_expired_session_cleanup() {
    // Create registry with very short TTL
    let registry = SessionRegistry::with_config(
        Duration::from_millis(50),
        Duration::from_secs(3600), // Long idle timeout
    );

    let peer_id = PeerId::random();
    let token = registry.create_session(peer_id);

    // Should be valid initially
    assert!(registry.validate(token.as_str()).is_some());

    // Wait for expiry
    std::thread::sleep(Duration::from_millis(100));

    // Should be invalid now
    assert!(registry.validate(token.as_str()).is_none());
}

#[test]
fn test_idle_session_cleanup() {
    // Create registry with very short idle timeout
    let registry = SessionRegistry::with_config(
        Duration::from_secs(3600), // Long TTL
        Duration::from_millis(50), // Short idle timeout
    );

    let peer_id = PeerId::random();
    let token = registry.create_session(peer_id);

    // Should be valid initially
    assert!(registry.validate(token.as_str()).is_some());

    // Touch to reset idle timer
    assert!(registry.validate(token.as_str()).is_some());

    // Wait for idle timeout
    std::thread::sleep(Duration::from_millis(100));

    // Should be invalid now due to idle timeout
    assert!(registry.validate(token.as_str()).is_none());
}

#[test]
fn test_session_token_format() {
    // Valid tokens
    assert!(SessionToken::from_hex(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ).is_some());

    // Case insensitive
    assert!(SessionToken::from_hex(
        "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789"
    ).is_some());

    // Invalid: wrong length
    assert!(SessionToken::from_hex("0123456789").is_none());
    assert!(SessionToken::from_hex("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef00").is_none());

    // Invalid: non-hex characters
    assert!(SessionToken::from_hex(
        "ghij56789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    ).is_none());
}

#[test]
fn test_concurrent_access() {
    use std::thread;

    let registry = SessionRegistry::new();
    let peers = ConnectedPeers::new();

    let registry_clone = registry.clone();
    let peers_clone = peers.clone();

    // Spawn threads that create/validate sessions
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let r = registry_clone.clone();
            let p = peers_clone.clone();

            thread::spawn(move || {
                let peer_id = PeerId::random();
                p.add(peer_id);
                let token = r.create_session(peer_id);
                
                // Multiple validations
                for _ in 0..10 {
                    assert!(r.validate(token.as_str()).is_some());
                }
                
                r.revoke_for_peer(&peer_id);
                p.remove(&peer_id);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(registry.session_count(), 0);
    assert_eq!(peers.count(), 0);
}
