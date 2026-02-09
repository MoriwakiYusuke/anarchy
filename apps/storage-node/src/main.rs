//! Anarchy Storage Node Daemon
//!
//! A distributed storage node for the Anarchy network.
//! Stores fragments and communicates via libp2p.

use anarchy_storage_node::{config, identity, storage, network, chain, rpc};

use clap::Parser;
use std::sync::Arc;
use tracing::{info, warn, error};
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
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("anarchy_storage_node=info".parse().unwrap())
                .add_directive("libp2p=warn".parse().unwrap()),
        )
        .init();

    // Parse CLI arguments
    let args = Args::parse();

    // Load configuration
    let overrides = config::ConfigOverrides {
        data_dir: args.data_dir.clone(),
        chain_url: args.chain_url.clone(),
        listen_addr: args.listen.clone(),
        rpc_port: args.rpc_port,
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

    // Initialize chain client (for declare_holding)
    let chain_client = chain::ChainClient::new(&config.chain_url, config.declare_rate_limit).await?;
    info!(endpoint = %config.chain_url, "Chain client initialized");

    // Initialize network
    let mut network = network::Network::new(identity.keypair().clone(), &config.listen_addr)?;
    network.listen(&config.listen_addr)?;
    info!("Network listening on {}", config.listen_addr);

    // Start HTTP RPC server
    let rpc_addr = format!("0.0.0.0:{}", config.rpc_port);
    let rpc_router = rpc::create_rpc_router(Arc::clone(&store));
    let rpc_listener = tokio::net::TcpListener::bind(&rpc_addr).await?;
    info!(addr = %rpc_addr, "HTTP RPC server started");
    
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
            warn!(error = %e, "Failed to register with blockchain node (will retry on reconnect)");
            // Continue anyway - blockchain node might not be running yet
        }
    }

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
