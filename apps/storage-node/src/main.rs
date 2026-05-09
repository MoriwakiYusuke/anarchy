//! Anarchy Storage Node Daemon
//!
//! A distributed storage node for the Anarchy network.
//! Stores fragments and communicates via libp2p.

use anarchy_storage_node::{config, identity, storage, network, chain, rpc, metrics::Metrics, prover, gc, challenge};

use clap::Parser;
use std::sync::Arc;
use tracing::{info, warn, error, debug};
use tokio::select;

/// Storage node CLI arguments
#[derive(Parser, Debug)]
#[command(name = "anarchy-storage-node")]
#[command(about = "Anarchy distributed storage node")]
pub struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    pub config: String,

    /// Data directory (overrides config)
    #[arg(short, long)]
    pub data_dir: Option<String>,

    /// Chain RPC URL (overrides config)
    #[arg(long)]
    pub chain_url: Option<String>,

    /// Listen address (overrides config)
    #[arg(long)]
    pub listen: Option<String>,

    /// HTTP RPC port (overrides config)
    #[arg(long)]
    pub rpc_port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging (NFR-001: structured JSON logs when ANARCHY_LOG_JSON=1)
    let use_json = std::env::var("ANARCHY_LOG_JSON")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);
        
    let env_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("anarchy_storage_node=info".parse().unwrap())
        .add_directive("libp2p=warn".parse().unwrap());
    
    if use_json {
        // NFR-001: JSON structured logging
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        // Human-readable format for development
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }

    // Parse CLI arguments
    let args = Args::parse();

    // Load configuration
    let overrides = config::ConfigOverrides {
        data_dir: args.data_dir.clone(),
        chain_url: args.chain_url.clone(),
        listen_addr: args.listen.clone(),
        rpc_port: args.rpc_port,
        auth_enabled: None,
    };
    let config = config::Config::load(&args.config, overrides)?;
    info!("Configuration loaded from {}", args.config);

    // Initialize identity (PeerID)
    let identity = identity::NodeIdentity::load_or_create(&config.data_dir)?;
    info!(peer_id = %identity.peer_id(), "Node identity loaded");

    // Initialize storage
    let store = Arc::new(storage::FragmentStore::new_with_verify(
        &config.data_dir,
        config.capacity,
        config.verify_on_read,
    )?);
    info!(
        capacity_bytes = config.capacity,
        verify_on_read = config.verify_on_read,
        "Fragment storage initialized"
    );

    // Initialize KZG prover with SRS (T082)
    let kzg_prover = Arc::new(prover::create_prover(&config.srs_path, config.dev_mode)?);
    if kzg_prover.is_srs_loaded() {
        info!(degree = kzg_prover.srs_degree(), "KZG prover initialized with SRS");
    } else {
        warn!("KZG prover initialized without SRS - proof generation will fail");
    }

    // Initialize garbage collector (T085)
    let garbage_collector = Arc::new(tokio::sync::Mutex::new(gc::GarbageCollector::new(config.dev_mode)));
    info!(dev_mode = config.dev_mode, "Garbage collector initialized");
    
    // Initialize stale holder GC (T059, T060 - 013-slashing-repair)
    let stale_holder_gc_config = gc::StaleHolderGcConfig::default();
    let stale_holder_gc_enabled = stale_holder_gc_config.enabled;
    let stale_holder_gc_interval_secs = stale_holder_gc_config.check_interval_secs;
    
    // GC store reference for deletion
    let gc_store = Arc::clone(&store);

    // Initialize metrics (NFR-001, NFR-002, NFR-003)
    let metrics = Metrics::new();
    metrics.set_capacity_total(config.capacity);
    info!("Metrics initialized");

    // Initialize endpoint cache for peer discovery (FR-507)
    // Using dev chain_id [0; 32] - in production this should be fetched from genesis
    let endpoint_cache = Arc::new(network::endpoint_cache::EndpointCache::new([0u8; 32]));
    info!("Endpoint cache initialized for peer discovery");
    
    // Initialize storage node cache for storage node address sharing (FR-515, FR-516)
    let storage_node_cache = Arc::new(network::storage_node_cache::StorageNodeCache::new());
    info!("Storage node cache initialized for address sharing");

    // Initialize failover manager (FR-510, FR-511)
    let failover_manager = Arc::new(chain::failover::FailoverManager::new());
    info!("Failover manager initialized");

    // Initialize chain client (for declare_holding)
    let chain_client = Arc::new(chain::ChainClient::new(
        &config.chain_url,
        config.declare_rate_limit,
        &config.signer_seed,
        Arc::clone(&failover_manager),
        Arc::clone(&endpoint_cache),
    ).await?);
    info!(endpoint = %config.chain_url, "Chain client initialized with failover support");

    // Initialize stale holder GC instance (T060 - 013-slashing-repair)
    let stale_holder_gc = Arc::new(gc::StaleHolderGc::new(
        gc::StaleHolderGcConfig::default(),
        Arc::clone(&chain_client),
    ));
    if stale_holder_gc_enabled {
        info!(interval_secs = stale_holder_gc_interval_secs, "Stale holder GC initialized");
    }

    // Initialize network
    let mut network = network::Network::new(identity.keypair().clone(), &config.listen_addr)?;
    network.listen(&config.listen_addr)?;
    network.subscribe_endpoints()?;
    network.subscribe_storage_nodes()?;  // FR-520: Subscribe to storage node topic
    info!("Network listening on {}", config.listen_addr);

    // Start HTTP RPC server
    let rpc_addr = format!("0.0.0.0:{}", config.rpc_port);
    let rpc_router = rpc::create_rpc_router(Arc::clone(&store), config.auth_enabled, metrics.clone());
    let rpc_listener = tokio::net::TcpListener::bind(&rpc_addr).await?;
    info!(addr = %rpc_addr, auth = config.auth_enabled, "HTTP RPC server started (NFR-002: /metrics endpoint enabled)");
    
    // Spawn HTTP server
    let http_server = tokio::spawn(async move {
        axum::serve(rpc_listener, rpc_router)
            .await
            .expect("HTTP server error");
    });

    // Register with blockchain node (auto-connection)
    let our_rpc_url = format!("http://127.0.0.1:{}", config.rpc_port);
    match chain_client.register_with_blockchain(&our_rpc_url).await {
        Ok(()) => info!("Registered with blockchain node"),
        Err(e) => {
            warn!(error = %e, "Failed to register with blockchain node (will retry periodically)");
            // Continue anyway - blockchain node might not be running yet
        }
    }

    // Setup periodic re-registration (heartbeat)
    // This ensures reconnection if blockchain node restarts
    let heartbeat_chain_client = chain_client.clone();
    let heartbeat_url = our_rpc_url.clone();
    let mut heartbeat_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    
    // Setup periodic storage node broadcast interval (FR-519, FR-515)
    // Broadcasts known storage nodes to peers every 60 seconds
    let mut storage_node_broadcast_interval = tokio::time::interval(std::time::Duration::from_secs(60));
    storage_node_broadcast_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Setup periodic GC check interval (T085)
    // Check for fragments ready for garbage collection every 60 seconds
    let mut gc_check_interval = tokio::time::interval(std::time::Duration::from_secs(60));
    gc_check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // TODO §4.9 final: periodic touch-buffer flush + LRU eviction trigger.
    // The touch buffer accumulates `last_accessed_at` updates from every
    // retrieve call. Flushing every 60s keeps it bounded while batching
    // hundreds of retrieves into a single write txn. LRU is checked at
    // the same cadence — if used > 95% capacity, evict to 85%.
    let mut touch_flush_interval = tokio::time::interval(std::time::Duration::from_secs(60));
    touch_flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let touch_store = Arc::clone(&store);
    let lru_store = Arc::clone(&store);
    // Guard against overlapping spawn_blocking jobs. If a flush + eviction
    // run takes longer than the 60s tick (large touch buffer or large DB),
    // we skip the next tick rather than piling up redb write-txn waiters.
    let touch_running = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Setup periodic stale holder GC check interval (T060 - 013-slashing-repair)
    // Check for fragments with excess holders every 5 minutes
    let mut stale_holder_gc_interval = tokio::time::interval(std::time::Duration::from_secs(stale_holder_gc_interval_secs));
    stale_holder_gc_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let stale_holder_gc_handle = Arc::clone(&stale_holder_gc);

    // Initialize ChallengeMonitor (T047, Issue 11 fix)
    // Monitors blockchain for ChallengeIssued events and auto-submits proofs
    let challenge_monitor = Arc::new(challenge::ChallengeMonitor::with_chain_client(
        chain_client.signer_account_id(),
        Arc::clone(&kzg_prover),
        Arc::clone(&chain_client),
    ));
    info!("ChallengeMonitor initialized for automatic proof submission");
    
    // Setup challenge polling interval (T047)
    // Poll for new challenges every 3 seconds
    let mut challenge_poll_interval = tokio::time::interval(std::time::Duration::from_secs(3));
    challenge_poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let challenge_chain_client = Arc::clone(&chain_client);

    // Setup shutdown signal
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C handler");
        info!("Shutdown signal received");
        let _ = shutdown_tx.send(());
    });

    // Main event loop
    info!("Storage node running. Press Ctrl+C to stop.");
    
    loop {
        select! {
            _ = &mut shutdown_rx => {
                info!("Shutting down...");
                http_server.abort();
                break;
            }
            _ = heartbeat_interval.tick() => {
                // Periodic re-registration to handle blockchain node restarts
                match heartbeat_chain_client.register_with_blockchain(&heartbeat_url).await {
                    Ok(()) => debug!("Heartbeat: re-registered with blockchain node"),
                    Err(e) => debug!(error = %e, "Heartbeat: blockchain node not available"),
                }
            }
            _ = storage_node_broadcast_interval.tick() => {
                // FR-519: Periodic broadcast of known storage nodes via Gossipsub
                let nodes = storage_node_cache.get_healthy_by_latency().await;
                if !nodes.is_empty() {
                    match network.broadcast_storage_nodes(nodes.clone()) {
                        Ok(()) => debug!(count = nodes.len(), "Broadcast storage node update"),
                        Err(e) => debug!(error = %e, "Failed to broadcast storage nodes"),
                    }
                }
            }
            _ = gc_check_interval.tick() => {
                // T085: Periodic garbage collection check (score-based)
                let mut gc = garbage_collector.lock().await;
                
                // Score-based GC: Check which local fragments are forgetting candidates on-chain
                // A fragment becomes a forgetting candidate when its score < threshold (reward = 0)
                match gc_store.list_fragments() {
                    Ok(local_fragments) if !local_fragments.is_empty() => {
                        // Query chain for forgetting candidate status
                        match chain_client.check_forgetting_candidates(local_fragments.clone()).await {
                            Ok(candidates) => {
                                for (content_hash, is_candidate) in candidates {
                                    if is_candidate {
                                        // Mark for GC with default score 0 (below threshold)
                                        gc.mark_forgetting_candidate(content_hash, 0);
                                    } else {
                                        // Remove from candidates if score recovered
                                        gc.unmark_forgetting_candidate(&content_hash);
                                    }
                                }
                            }
                            Err(e) => debug!(error = %e, "GC: Failed to check forgetting candidates"),
                        }
                    }
                    Ok(_) => {
                        // No local fragments, nothing to check
                    }
                    Err(e) => debug!(error = %e, "GC: Failed to list local fragments"),
                }
                
                // Score-based GC: process fragments with expired grace period
                let ready_for_gc = gc.get_gc_ready();
                
                if !ready_for_gc.is_empty() {
                    info!(count = ready_for_gc.len(), "GC: Processing {} fragments ready for cleanup", ready_for_gc.len());
                    
                    for content_hash in ready_for_gc {
                        if gc.execute_gc(&content_hash) {
                            // Delete fragment from local storage
                            match gc_store.delete(&content_hash) {
                                Ok(true) => info!(
                                    hash = %hex::encode(&content_hash[..8]),
                                    "GC: Successfully deleted fragment"
                                ),
                                Ok(false) => debug!(
                                    hash = %hex::encode(&content_hash[..8]),
                                    "GC: Fragment not found (already deleted)"
                                ),
                                Err(e) => error!(
                                    error = %e,
                                    hash = %hex::encode(&content_hash[..8]),
                                    "GC: Failed to delete fragment"
                                ),
                            }
                        }
                    }
                }
            }
            // TODO §4.9 final: Touch buffer flush + LRU eviction trigger.
            //
            // 1) Flush all `last_accessed_at` updates queued during the
            //    last 60s into one write txn. Skipping a flush is OK —
            //    the buffer just gets bigger; the next tick catches up.
            //
            // 2) If usage crossed 95% of capacity, evict LRU down to 85%.
            //    The 10-percentage-point gap prevents thrashing where the
            //    next write trips the threshold again immediately.
            //
            // The `touch_running` flag prevents overlapping spawn_blocking
            // jobs when a flush+eviction run takes longer than 60s — they
            // would otherwise queue up, contending on redb write txns.
            _ = touch_flush_interval.tick() => {
                use std::sync::atomic::Ordering;
                if touch_running
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_err()
                {
                    debug!("touch flush + eviction still running from previous tick — skipping");
                } else {
                    let touch_handle = Arc::clone(&touch_store);
                    let lru_handle = Arc::clone(&lru_store);
                    let running_flag = Arc::clone(&touch_running);
                    tokio::task::spawn_blocking(move || {
                        // Always release the flag on scope exit, including
                        // panic-unwind. Otherwise a single panicking flush
                        // would prevent every future tick from running.
                        struct ReleaseOnDrop(Arc<std::sync::atomic::AtomicBool>);
                        impl Drop for ReleaseOnDrop {
                            fn drop(&mut self) {
                                self.0.store(false, Ordering::Release);
                            }
                        }
                        let _guard = ReleaseOnDrop(running_flag);

                        match touch_handle.flush_touch_buffer() {
                            Ok(0) => {}
                            Ok(n) => debug!(touched = n, "Flushed touch buffer"),
                            Err(e) => warn!(error = %e, "Failed to flush touch buffer"),
                        }

                        let used = lru_handle.used_bytes();
                        let cap = lru_handle.capacity_bytes();
                        // 95% threshold via division to avoid `cap * 95` overflow on
                        // very large nodes (u64::MAX / 95 ≈ 194 PB).
                        let trigger = cap / 100 * 95;
                        if cap > 0 && used > trigger {
                            // Aim for ~85% afterwards; same overflow-safe form.
                            let target = cap / 100 * 85;
                            match lru_handle.evict_lru(target) {
                                Ok(stats) if stats.bytes_freed > 0 => {
                                    info!(
                                        fragments = stats.fragments_evicted,
                                        post_fragments = stats.post_fragments_evicted,
                                        bytes_freed = stats.bytes_freed,
                                        used_after = lru_handle.used_bytes(),
                                        capacity = cap,
                                        "LRU eviction: capacity > 95%, freed to ~85%"
                                    );
                                }
                                Ok(_) => {}
                                Err(e) => warn!(error = %e, "LRU eviction failed"),
                            }
                        }
                    });
                }
            }
            // T060: Stale holder GC polling (013-slashing-repair)
            _ = stale_holder_gc_interval.tick() => {
                if stale_holder_gc_enabled {
                    match stale_holder_gc_handle.run_cycle().await {
                        Ok(evicted) if evicted > 0 => {
                            info!(count = evicted, "Stale holder GC: evicted {} excess holders", evicted);
                        }
                        Ok(_) => {
                            debug!("Stale holder GC: no excess holders to evict");
                        }
                        Err(e) => {
                            debug!(error = %e, "Stale holder GC: failed to run cycle");
                        }
                    }
                }
            }
            // T047/T048: ChallengeMonitor polling (Issue 11 fix)
            _ = challenge_poll_interval.tick() => {
                // Poll blockchain for new challenges targeting our account
                match challenge_chain_client.poll_challenges().await {
                    Ok(challenges) => {
                        for challenge in challenges {
                            // Process each challenge via ChallengeMonitor
                            if let Err(e) = challenge_monitor.on_challenge_issued(challenge).await {
                                warn!(error = %e, "Failed to process challenge");
                            }
                        }
                    }
                    Err(e) => {
                        debug!(error = %e, "Failed to poll challenges (chain may be unavailable)");
                    }
                }
                
                // Process any pending retries
                if let Err(e) = challenge_monitor.process_pending().await {
                    debug!(error = %e, "Failed to process pending challenges");
                }
            }
            event = network.handle_event(store.as_ref(), chain_client.as_ref()) => {
                match event {
                    Ok(Some(network::NetworkEvent::Listening(addr))) => {
                        info!(addr = %addr, "Now listening on");
                    }
                    Ok(Some(network::NetworkEvent::PeerConnected(peer))) => {
                        info!(peer = %peer, "Peer connected");
                    }
                    Ok(Some(network::NetworkEvent::PeerDisconnected(peer))) => {
                        info!(peer = %peer, "Peer disconnected");
                    }
                    Ok(Some(network::NetworkEvent::FragmentResponse { peer, response })) => {
                        info!(peer = %peer, "Received fragment response: {:?}", response);
                        // TODO: Handle response (e.g., store received fragment)
                    }
                    Ok(Some(network::NetworkEvent::FragmentStored { fragment_id })) => {
                        // Auto-declare holding on successful PUT (T056)
                        info!(fragment_id = %hex::encode(fragment_id), "Fragment stored, triggering auto-declare");
                        if let Err(e) = chain_client.declare_holding(fragment_id).await {
                            error!(error = %e, "Failed to declare holding (rate limited?)");
                        }
                    }
                    Ok(Some(network::NetworkEvent::EndpointUpdate { from, endpoints })) => {
                        info!(
                            peer = %from,
                            endpoint_count = endpoints.len(),
                            "Received endpoint update via gossipsub"
                        );
                        // Update local endpoint cache for discovery (FR-507)
                        let cache = endpoint_cache.clone();
                        for ep in endpoints {
                            if cache.insert(ep.clone()).await {
                                debug!(url = %ep.url, "Added endpoint to cache");
                            }
                        }
                    }
                    Ok(Some(network::NetworkEvent::StorageNodeUpdate { from, nodes })) => {
                        // FR-515, FR-519: Received storage node addresses via gossipsub
                        info!(
                            peer = %from,
                            node_count = nodes.len(),
                            "Received storage node update via gossipsub"
                        );
                        // Update local storage node cache
                        let cache = storage_node_cache.clone();
                        for node in nodes {
                            if cache.insert(node.clone()).await {
                                debug!(url = %node.url, "Added storage node to cache");
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        error!("Network error: {}", e);
                    }
                }
            }
        }
    }
    
    info!("Storage node stopped");
    Ok(())
}
