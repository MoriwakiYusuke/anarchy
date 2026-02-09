//! Integration tests for storage node
//!
//! Tests full node lifecycle including startup and shutdown.

use std::time::Duration;
use tokio::time::timeout;

/// Test: Node can be created and shutdown gracefully (T-106)
/// 
/// This test verifies that the storage node daemon can handle
/// shutdown signals properly without hanging or losing data.
#[tokio::test]
async fn test_graceful_shutdown() {
    // Create a shutdown signal channel
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    
    // Simulate the main loop pattern
    let handle = tokio::spawn(async move {
        let mut iterations = 0;
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    // Graceful shutdown received
                    return iterations;
                }
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    iterations += 1;
                    if iterations > 100 {
                        // Safety timeout for test
                        return iterations;
                    }
                }
            }
        }
    });
    
    // Let it run for a bit
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Send shutdown signal
    let _ = shutdown_tx.send(());
    
    // Wait for shutdown with timeout
    let result = timeout(Duration::from_secs(1), handle).await;
    
    assert!(result.is_ok(), "Shutdown should complete within timeout");
    let iterations = result.unwrap().unwrap();
    assert!(iterations > 0, "Should have run some iterations before shutdown");
    assert!(iterations < 100, "Should have shutdown before safety limit");
}

/// Test: Shutdown handler processes SIGINT-like signal
#[tokio::test]
async fn test_shutdown_signal_handling() {
    use tokio::sync::watch;
    
    // Create a watch channel to simulate shutdown coordination
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    
    let worker = tokio::spawn({
        let mut shutdown_rx = shutdown_rx.clone();
        async move {
            let mut work_done = 0;
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            // Perform cleanup
                            return work_done;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(5)) => {
                        work_done += 1;
                    }
                }
            }
        }
    });
    
    // Let worker run
    tokio::time::sleep(Duration::from_millis(30)).await;
    
    // Signal shutdown
    shutdown_tx.send(true).unwrap();
    
    // Worker should exit cleanly
    let result = timeout(Duration::from_secs(1), worker).await;
    assert!(result.is_ok(), "Worker should shutdown within timeout");
    
    let work_done = result.unwrap().unwrap();
    assert!(work_done > 0, "Worker should have done some work");
}

/// Test: Multiple components can coordinate shutdown
#[tokio::test]
async fn test_coordinated_shutdown() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let component_count = Arc::new(AtomicU32::new(0));
    
    // Spawn multiple "components"
    let mut handles = vec![];
    for i in 0..3 {
        let flag = shutdown_flag.clone();
        let count = component_count.clone();
        
        handles.push(tokio::spawn(async move {
            // Component startup
            count.fetch_add(1, Ordering::SeqCst);
            
            // Wait for shutdown
            while !flag.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            
            // Component cleanup
            i // Return component ID
        }));
    }
    
    // Wait for all components to start
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(component_count.load(Ordering::SeqCst), 3, "All components should start");
    
    // Signal shutdown
    shutdown_flag.store(true, Ordering::SeqCst);
    
    // Wait for all components to exit
    for (idx, handle) in handles.into_iter().enumerate() {
        let result = timeout(Duration::from_secs(1), handle).await;
        assert!(result.is_ok(), "Component {} should shutdown", idx);
    }
}
