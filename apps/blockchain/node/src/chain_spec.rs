//! Chain Specification
//!
//! 開発用およびテストネット用のチェーン仕様を定義

use anarchy_runtime::{
    opaque::SessionKeys, AccountId, AuraConfig, Balance, BalancesConfig, GrandpaConfig,
    RuntimeGenesisConfig, Signature, SudoConfig, SystemConfig, WASM_BINARY,
};
use sc_service::ChainType;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
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

/// Authorityのキーペアを生成
pub fn authority_keys_from_seed(s: &str) -> (AuraId, GrandpaId) {
    (get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s))
}

/// チェーンのプロパティ（トークン情報）
fn chain_properties() -> sc_service::Properties {
    let mut properties = sc_service::Properties::new();
    properties.insert("tokenSymbol".into(), "MORAL".into());
    properties.insert("tokenDecimals".into(), 12.into());
    properties.insert("ss58Format".into(), 42.into());
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
        // 初期オーソリティ
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
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Local wasm not available".to_string())?,
        None,
    )
    .with_name("Local Testnet")
    .with_id("local_testnet")
    .with_chain_type(ChainType::Local)
    .with_properties(chain_properties())
    .with_genesis_config_patch(testnet_genesis(
        vec![
            authority_keys_from_seed("Alice"),
            authority_keys_from_seed("Bob"),
        ],
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

/// Genesisコンフィグを構築
fn testnet_genesis(
    initial_authorities: Vec<(AuraId, GrandpaId)>,
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
    _enable_println: bool,
) -> serde_json::Value {
    // $moral = ネイティブトークン (12桁精度)
    const INITIAL_MORAL: Balance = 10_000_000_000_000_000; // 10,000 $moral

    serde_json::json!({
        "balances": {
            "balances": endowed_accounts.iter().cloned().map(|k| (k, INITIAL_MORAL)).collect::<Vec<_>>()
        },
        "aura": {
            "authorities": initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>()
        },
        "grandpa": {
            "authorities": initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect::<Vec<_>>()
        },
        "sudo": {
            "key": Some(root_key)
        }
    })
}
