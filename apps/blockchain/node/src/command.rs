//! コマンド実行

use crate::{
    chain_spec,
    cli::{Cli, Subcommand, TorMode},
    service,
};
use clap::Parser;
use sc_cli::SubstrateCli;
use sc_service::PartialComponents;

/// Environment variable that indicates the node is running under torsocks wrapper
const TORSOCKS_ENV_VAR: &str = "ANARCHY_RUNNING_UNDER_TORSOCKS";

/// Check if running under torsocks wrapper
/// Detects either:
/// 1. Our custom environment variable (ANARCHY_RUNNING_UNDER_TORSOCKS)
/// 2. LD_PRELOAD containing "torsocks" (set by torsocks command)
fn is_running_under_torsocks() -> bool {
    // Check our custom variable first
    if std::env::var(TORSOCKS_ENV_VAR).is_ok() {
        return true;
    }
    // Check LD_PRELOAD for torsocks library
    if let Ok(ld_preload) = std::env::var("LD_PRELOAD") {
        if ld_preload.to_lowercase().contains("torsocks") {
            return true;
        }
    }
    false
}

/// Sanitize Onion addresses in a string for privacy-safe logging
/// 
/// Replaces the full Onion address with a truncated version.
/// Example: /onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333
///       -> /onion3/vww6yb...pnpyyd:30333
#[allow(dead_code)]
fn sanitize_onion_in_string(s: &str) -> String {
    // Match /onion3/<56-chars>:<port> pattern
    let mut result = s.to_string();
    
    // Simple regex-free approach: find /onion3/ and truncate the address
    if let Some(start) = result.find("/onion3/") {
        let after_prefix = start + 8; // len("/onion3/")
        if result.len() >= after_prefix + 56 {
            // Extract first 6 and last 6 characters of the base32 part
            let addr_start = &result[after_prefix..after_prefix + 6];
            let addr_end = &result[after_prefix + 50..after_prefix + 56];
            
            // Find the colon after the address
            if let Some(colon_pos) = result[after_prefix..].find(':') {
                let port_part = &result[after_prefix + colon_pos..];
                // Extract just the port number portion (up to next / or end)
                let port_end = port_part.find('/').unwrap_or(port_part.len());
                let port_suffix = &port_part[..port_end];
                let rest = &port_part[port_end..];
                
                result = format!(
                    "{}/onion3/{}...{}{}{}",
                    &s[..start],
                    addr_start,
                    addr_end,
                    port_suffix,
                    rest
                );
            }
        }
    }
    
    result
}

/// Validate Onion v3 address format
/// 
/// Onion v3 addresses are 56 characters of base32 (a-z, 2-7)
/// Example: /onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333
fn validate_onion_address(addr: &str) -> Result<(), String> {
    // Check for /onion3/ prefix
    let onion_part = if let Some(rest) = addr.strip_prefix("/onion3/") {
        rest
    } else {
        return Err(format!("Onion address must start with /onion3/: {}", addr));
    };
    
    // Split address and port
    let (base32_part, _port) = match onion_part.split_once(':') {
        Some(parts) => parts,
        None => return Err(format!("Onion address must include port: {}", addr)),
    };
    
    // Validate base32 format: 56 characters of a-z and 2-7
    if base32_part.len() != 56 {
        return Err(format!(
            "Onion v3 address must be 56 characters, got {}: {}",
            base32_part.len(),
            addr
        ));
    }
    
    for c in base32_part.chars() {
        if !matches!(c, 'a'..='z' | '2'..='7') {
            return Err(format!(
                "Invalid character '{}' in Onion address. Must be a-z or 2-7: {}",
                c, addr
            ));
        }
    }
    
    Ok(())
}

/// Validate public addresses for Onion format if applicable
fn validate_public_addresses(config: &sc_service::Configuration) -> Result<(), String> {
    for addr in &config.network.public_addresses {
        let addr_str = addr.to_string();
        if addr_str.contains("/onion3/") {
            validate_onion_address(&addr_str)?;
        }
    }
    Ok(())
}

/// Apply Tor mode configuration to network settings.
///
/// Returns `Err` instead of `process::exit` on misconfiguration so that the
/// caller can shut down RocksDB and other resources gracefully (#28-CRIT-2).
fn apply_tor_mode(
    tor_mode: TorMode,
    config: &mut sc_service::Configuration,
) -> Result<(), sc_cli::Error> {
    match tor_mode {
        TorMode::Off => {
            log::info!("🌐 Tor mode: OFF - Direct TCP connections (development only)");
        }
        TorMode::OutboundOnly => {
            log::warn!("⚠️  Tor mode: OUTBOUND-ONLY - Your inbound IP is EXPOSED!");
            log::warn!("⚠️  Use --tor-mode=forced for full anonymity in production");

            if !is_running_under_torsocks() {
                log::warn!("⚠️  Not running under torsocks. Outbound traffic may leak!");
                log::warn!("⚠️  Use: ./scripts/anarchy-tor.sh ./anarchy-node --tor-mode=outbound-only");
            }
        }
        TorMode::Forced => {
            // ① Outbound Lock: Require torsocks environment
            if !is_running_under_torsocks() {
                return Err(sc_cli::Error::Input(
                    "Tor mode FORCED requires running under torsocks. \
                     Usage: ./scripts/anarchy-tor.sh ./anarchy-node --tor-mode=forced"
                        .to_string(),
                ));
            }

            // ② Inbound Lock: Force listen on localhost only
            let localhost_addr: sc_network::Multiaddr = "/ip4/127.0.0.1/tcp/30333"
                .parse()
                .expect("Valid multiaddr");

            config.network.listen_addresses = vec![localhost_addr];

            log::info!("🔒 Tor mode: FORCED - Full anonymity enabled");
            log::info!("🔒 ① Outbound Lock: All traffic via Tor (torsocks detected)");
            log::info!("🔒 ② Inbound Lock: Listening on 127.0.0.1:30333 only (Onion Service required)");
        }
    }
    Ok(())
}

impl SubstrateCli for Cli {
    fn impl_name() -> String {
        "Anarchy Node".into()
    }

    fn impl_version() -> String {
        env!("SUBSTRATE_CLI_IMPL_VERSION").into()
    }

    fn description() -> String {
        "Anarchy: 匿名分散型SNSノード".into()
    }

    fn author() -> String {
        env!("CARGO_PKG_AUTHORS").into()
    }

    fn support_url() -> String {
        "https://github.com/anarchy-project/anarchy/issues".into()
    }

    fn copyright_start_year() -> i32 {
        2024
    }

    fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
        Ok(match id {
            "dev" => Box::new(chain_spec::development_config()?),
            "" | "local" => Box::new(chain_spec::local_testnet_config()?),
            path => Box::new(chain_spec::ChainSpec::from_json_file(
                std::path::PathBuf::from(path),
            )?),
        })
    }
}

pub fn run() -> sc_cli::Result<()> {
    let cli = Cli::parse();

    match &cli.subcommand {
        Some(Subcommand::Key(cmd)) => cmd.run(&cli),
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
        }
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    import_queue,
                    ..
                } = service::new_partial(&config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    ..
                } = service::new_partial(&config)?;
                Ok((cmd.run(client, config.database), task_manager))
            })
        }
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    ..
                } = service::new_partial(&config)?;
                Ok((cmd.run(client, config.chain_spec), task_manager))
            })
        }
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    import_queue,
                    ..
                } = service::new_partial(&config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.database))
        }
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    backend,
                    ..
                } = service::new_partial(&config)?;
                Ok((cmd.run(client, backend, None), task_manager))
            })
        }
        None => {
            let tor_mode = cli.tor_mode;
            let runner = cli.create_runner(&cli.run)?;
            runner.run_node_until_exit(|mut config| async move {
                // Validate Onion addresses in --public-addr if any
                if let Err(e) = validate_public_addresses(&config) {
                    log::error!("🚫 Invalid public address: {}", e);
                    return Err(sc_cli::Error::Input(e));
                }
                
                // Mainnet requires forced Tor mode - override any user setting
                let effective_tor_mode = if config.chain_spec.id().contains("mainnet") {
                    if tor_mode != TorMode::Forced {
                        log::warn!("⚠️  Mainnet requires --tor-mode=forced. Overriding user setting.");
                    }
                    TorMode::Forced
                } else {
                    tor_mode
                };
                
                // Apply Tor mode configuration
                apply_tor_mode(effective_tor_mode, &mut config)?;

                service::new_full(config).map_err(sc_cli::Error::Service)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // validate_onion_address() tests
    // ============================================================

    #[test]
    fn test_valid_onion_address() {
        // Valid v3 Onion address (56 chars base32)
        let addr = "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333";
        assert!(validate_onion_address(addr).is_ok());
    }

    #[test]
    fn test_valid_onion_address_different_port() {
        let addr = "/onion3/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2:9944";
        assert!(validate_onion_address(addr).is_ok());
    }

    #[test]
    fn test_invalid_onion_missing_prefix() {
        let addr = "vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333";
        let result = validate_onion_address(addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("/onion3/"));
    }

    #[test]
    fn test_invalid_onion_missing_port() {
        let addr = "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd";
        let result = validate_onion_address(addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("port"));
    }

    #[test]
    fn test_invalid_onion_too_short() {
        // Only 50 characters instead of 56
        let addr = "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah:30333";
        let result = validate_onion_address(addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("56 characters"));
    }

    #[test]
    fn test_invalid_onion_too_long() {
        // 60 characters instead of 56
        let addr = "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyydaaaa:30333";
        let result = validate_onion_address(addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("56 characters"));
    }

    #[test]
    fn test_invalid_onion_bad_characters() {
        // Contains '8' which is not valid base32 (only a-z and 2-7)
        let addr = "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpy8d:30333";
        let result = validate_onion_address(addr);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid character"));
    }

    #[test]
    fn test_invalid_onion_uppercase() {
        // Base32 should be lowercase
        let addr = "/onion3/VWW6YBAL4BD7SZMGNCYRUUCPGFKQAHZDDI37KTCEO3AH7NGMCOPNPYYD:30333";
        let result = validate_onion_address(addr);
        assert!(result.is_err());
    }

    // ============================================================
    // sanitize_onion_in_string() tests
    // ============================================================

    #[test]
    fn test_sanitize_onion_basic() {
        let input = "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333";
        let result = sanitize_onion_in_string(input);
        assert!(result.contains("vww6yb"));
        assert!(result.contains("pnpyyd"));
        assert!(result.contains("..."));
        assert!(!result.contains("szmgncyruucpgfkqahzddi37ktceo3ah7ngmco"));
    }

    #[test]
    fn test_sanitize_onion_with_peer_id() {
        let input = "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp";
        let result = sanitize_onion_in_string(input);
        assert!(result.contains("/p2p/12D3KooW"));
        assert!(result.contains("..."));
    }

    #[test]
    fn test_sanitize_no_onion() {
        let input = "/ip4/127.0.0.1/tcp/30333";
        let result = sanitize_onion_in_string(input);
        assert_eq!(input, result);
    }

    #[test]
    fn test_sanitize_empty_string() {
        let result = sanitize_onion_in_string("");
        assert_eq!("", result);
    }

    // ============================================================
    // is_running_under_torsocks() tests
    // ============================================================

    #[test]
    fn test_torsocks_env_not_set() {
        // Ensure both env vars are not set
        std::env::remove_var(TORSOCKS_ENV_VAR);
        std::env::remove_var("LD_PRELOAD");
        assert!(!is_running_under_torsocks());
    }

    #[test]
    fn test_torsocks_env_set() {
        // Set the env var
        std::env::set_var(TORSOCKS_ENV_VAR, "1");
        assert!(is_running_under_torsocks());
        // Clean up
        std::env::remove_var(TORSOCKS_ENV_VAR);
    }

    #[test]
    fn test_torsocks_env_empty_value() {
        // Even empty value should return true (var exists)
        std::env::set_var(TORSOCKS_ENV_VAR, "");
        assert!(is_running_under_torsocks());
        // Clean up
        std::env::remove_var(TORSOCKS_ENV_VAR);
    }

    #[test]
    fn test_torsocks_ld_preload_detection() {
        // Ensure our custom var is not set
        std::env::remove_var(TORSOCKS_ENV_VAR);
        
        // Set LD_PRELOAD with torsocks library (as torsocks command does)
        std::env::set_var("LD_PRELOAD", "/usr/lib/x86_64-linux-gnu/torsocks/libtorsocks.so");
        assert!(is_running_under_torsocks());
        
        // Test case-insensitive matching
        std::env::set_var("LD_PRELOAD", "/path/to/LIBTORSOCKS.so");
        assert!(is_running_under_torsocks());
        
        // Clean up
        std::env::remove_var("LD_PRELOAD");
    }

    #[test]
    fn test_torsocks_ld_preload_non_torsocks() {
        // Ensure our custom var is not set
        std::env::remove_var(TORSOCKS_ENV_VAR);
        
        // LD_PRELOAD without torsocks should return false
        std::env::set_var("LD_PRELOAD", "/some/other/library.so");
        assert!(!is_running_under_torsocks());
        
        // Clean up
        std::env::remove_var("LD_PRELOAD");
    }

    // ============================================================
    // TorMode enum tests
    // ============================================================

    #[test]
    fn test_tor_mode_default() {
        assert_eq!(TorMode::default(), TorMode::Off);
    }

    #[test]
    fn test_tor_mode_equality() {
        assert_eq!(TorMode::Off, TorMode::Off);
        assert_eq!(TorMode::OutboundOnly, TorMode::OutboundOnly);
        assert_eq!(TorMode::Forced, TorMode::Forced);
        assert_ne!(TorMode::Off, TorMode::Forced);
    }
}
