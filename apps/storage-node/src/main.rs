//! Anarchy Storage Node Daemon
//!
//! A distributed storage node for the Anarchy network.
//! Stores fragments and communicates via libp2p.

use anarchy_storage_node::{config, identity, storage, network, chain, rpc, metrics::Metrics};

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
    let store = Arc::new(storage::FragmentStore::new(&config.data_dir, config.capacity)?);
    info!(
        capacity_bytes = config.capacity,
        "Fragment storage initialized"
    );

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
        Arc::clone(&failover_manager),
        Arc::clone(&endpoint_cache),
    ).await?);
    info!(endpoint = %config.chain_url, "Chain client initialized with failover support");

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
            event = network.handle_event(store.as_ref()) => {
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
