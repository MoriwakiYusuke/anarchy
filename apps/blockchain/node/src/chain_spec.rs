//! Chain Specification
//!
//! 開発用およびテストネット用のチェーン仕様を定義
//!
//! ## Tor/Onion Bootstrap Nodes
//!
//! Onion multiaddress format for bootNodes:
//! ```
//! /onion3/<56-char-base32>:<port>/p2p/<peer-id>
//! ```
//!
//! Example:
//! ```
//! "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"
//! ```
//!
//! Production networks should include both TCP and Onion bootnodes for
//! maximum reachability and privacy options.

use anarchy_runtime::{AccountId, Balance, Signature, WASM_BINARY};
use sc_service::ChainType;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

/// 特殊なChainSpec型
pub type ChainSpec = sc_service::GenericChainSpec;

/// 公開鍵を生成するヘルパー
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// AccountIdを生成
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Authorityのキーペアを生成 (PoW移行後はGrandpa keyのみ)
pub fn authority_keys_from_seed(s: &str) -> GrandpaId {
    get_from_seed::<GrandpaId>(s)
}

/// チェーンのプロパティ（トークン情報）
fn chain_properties() -> sc_service::Properties {
    let mut properties = sc_service::Properties::new();
    // トークン設定
    properties.insert("tokenSymbol".into(), "MORAL".into());
    properties.insert("tokenDecimals".into(), 12.into());
    properties.insert("ss58Format".into(), 42.into());
    
    // ネットワーク情報
    properties.insert("displayName".into(), "Anarchy Network".into());
    properties.insert("isTestnet".into(), true.into());
    
    // 外部リンク（TODO: 本番URLが決まったら更新）
    // properties.insert("faucetUrl".into(), "http://anarchy-faucet.onion".into());
    // properties.insert("website".into(), "http://anarchy-social.onion".into());
    
    properties
}

/// 開発用設定
pub fn development_config() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        None,
    )
    .with_name("Development")
    .with_id("dev")
    .with_chain_type(ChainType::Development)
    .with_properties(chain_properties())
    .with_genesis_config_patch(testnet_genesis(
        // 初期 GRANDPA オーソリティ (PoW 移行後は Aura 不要)
        vec![authority_keys_from_seed("Alice")],
        // Sudoアカウント
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        // 事前資金付与アカウント
        vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
            get_account_id_from_seed::<sr25519::Public>("Charlie"),
            get_account_id_from_seed::<sr25519::Public>("Dave"),
            get_account_id_from_seed::<sr25519::Public>("Eve"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie"),
            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
        ],
        true,
    ))
    .build())
}

/// ローカルテストネット設定
pub fn local_testnet_config() -> Result<ChainSpec, String> {
    // Boot nodes for network discovery
    // For production, add both TCP and Onion addresses
    // 
    // Onion bootnode format:
    //   /onion3/<56-char-base32>:<port>/p2p/<peer-id>
    // 
    // Example entries (uncomment and replace with real addresses):
    // let boot_nodes = vec![
    //     // TCP bootnode (clear net)
    //     "/ip4/1.2.3.4/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"
    //         .parse()
    //         .expect("valid multiaddr"),
    //     // Onion bootnode (Tor hidden service)
    //     "/onion3/vww6ybal4bd7szmgncyruucpgfkqahzddi37ktceo3ah7ngmcopnpyyd:30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"
    //         .parse()
    //         .expect("valid multiaddr"),
    // ];
    
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Local wasm not available".to_string())?,
        None,
    )
    .with_name("Local Testnet")
    .with_id("local_testnet")
    .with_chain_type(ChainType::Local)
    .with_properties(chain_properties())
    // For production networks, uncomment:
    // .with_boot_nodes(boot_nodes)
    .with_genesis_config_patch(testnet_genesis(
        vec![
            authority_keys_from_seed("Alice"),
            authority_keys_from_seed("Bob"),
        ], // GRANDPA authority list
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
            get_account_id_from_seed::<sr25519::Public>("Charlie"),
            get_account_id_from_seed::<sr25519::Public>("Dave"),
            get_account_id_from_seed::<sr25519::Public>("Eve"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie"),
            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
        ],
        true,
    ))
    .build())
}

/// Genesisコンフィグを構築 (PoW 移行後: Aura 削除、GRANDPA authority のみ設定)
fn testnet_genesis(
    initial_grandpa_authorities: Vec<GrandpaId>,
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
) -> serde_json::Value {
    // $moral = ネイティブトークン (12桁精度)
    const INITIAL_MORAL: Balance = 10_000_000_000_000_000; // 10,000 $moral

    // 報酬プール初期残高 (TSTS v1: 100,000 MORAL に縮小).
    // Block reward 30% 流入で運用中に充填されるので genesis seed は小さくて十分。
    const INITIAL_REWARD_POOL: u128 = 100_000_000_000_000_000; // 100,000 $moral

    // Reaction報酬プール初期残高 (TSTS v1: 100,000 MORAL に縮小).
    // 動的 γ × decay × per-block cap で自己平衡するので genesis seed は小さくて十分。
    const INITIAL_REACTION_REWARD_POOL: u128 = 100_000_000_000_000_000; // 100,000 $moral

    // Reaction初期難易度 (BaseDifficultyと同じ)
    const INITIAL_REACTION_DIFFICULTY: u8 = 16;

    serde_json::json!({
        "balances": {
            "balances": endowed_accounts.iter().cloned().map(|k| (k, INITIAL_MORAL)).collect::<Vec<_>>()
        },
        "grandpa": {
            "authorities": initial_grandpa_authorities.iter().map(|k| (k.clone(), 1)).collect::<Vec<_>>()
        },
        "sudo": {
            "key": Some(root_key)
        },
        "storage": {
            "initialRewardPool": INITIAL_REWARD_POOL
        },
        "reaction": {
            "initialRewardPool": INITIAL_REACTION_REWARD_POOL,
            "initialDifficulty": INITIAL_REACTION_DIFFICULTY
        }
    })
}
