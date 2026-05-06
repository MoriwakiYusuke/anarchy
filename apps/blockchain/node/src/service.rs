//! ノードサービス
//!
//! ノードの起動とサービス構成

use anarchy_runtime::{self, opaque::Block, RuntimeApi};
use futures::FutureExt;
use log::info;
use sc_client_api::{Backend, BlockBackend};
use sc_consensus_grandpa::SharedVoterState;
use sc_service::{error::Error as ServiceError, Configuration, TaskManager, WarpSyncConfig};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_runtime::traits::Block as BlockT;
use std::{sync::Arc, time::Duration};

/// 完全なクライアント型
pub type FullClient = sc_service::TFullClient<
    Block,
    RuntimeApi,
    sc_executor::WasmExecutor<sp_io::SubstrateHostFunctions>,
>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type FullPool = sc_transaction_pool::BasicPool<
    sc_transaction_pool::FullChainApi<FullClient, Block>,
    Block,
>;

/// 部分的なノードコンポーネントを構築
pub fn new_partial(
    config: &Configuration,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        sc_consensus::DefaultImportQueue<Block>,
        FullPool,
        (
            sc_consensus_grandpa::GrandpaBlockImport<
                FullBackend,
                Block,
                FullClient,
                FullSelectChain,
            >,
            sc_consensus_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
            Option<Telemetry>,
        ),
    >,
    ServiceError,
> {
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = sc_service::new_wasm_executor(&config.executor);

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager
            .spawn_handle()
            .spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        sc_transaction_pool::Options::default(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
        client.clone(),
        512,
        &client,
        select_chain.clone(),
        telemetry.as_ref().map(|x: &Telemetry| x.handle()),
    )?;

    // PoW インポートキュー (Phase B): Aura の代わりに sc_consensus_pow を使用。
    // import_queue では full_mode 不要 (verify のみ使用) なので light mode 固定。
    let pow_algo = crate::pow::RandomXAlgorithm::new(client.clone(), false);

    let import_queue = sc_consensus_pow::import_queue(
        Box::new(grandpa_block_import.clone()),
        None,
        pow_algo.clone(),
        &task_manager.spawn_essential_handle(),
        config.prometheus_registry(),
    )?;

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool: Arc::new(transaction_pool),
        other: (grandpa_block_import, grandpa_link, telemetry),
    })
}

/// フルノードを構築
pub fn new_full(
    config: Configuration,
    mine: bool,
    coinbase: Option<String>,
    randomx_mode: crate::cli::RandomxMode,
) -> Result<TaskManager, ServiceError> {
    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (block_import, grandpa_link, mut telemetry),
    } = new_partial(&config)?;

    let genesis_hash: <Block as BlockT>::Hash = client
        .block_hash(0u32)?
        .expect("Genesis block exists; qed");

    let grandpa_protocol_name = sc_consensus_grandpa::protocol_standard_name(
        &genesis_hash,
        &config.chain_spec,
    );

    let mut net_config =
        sc_network::config::FullNetworkConfiguration::<Block, <Block as BlockT>::Hash, sc_network::NetworkWorker<Block, <Block as BlockT>::Hash>>::new(
            &config.network,
            config.prometheus_registry().cloned(),
        );

    let metrics = sc_network::NotificationMetrics::new(config.prometheus_registry());
    let peer_store_handle = net_config.peer_store_handle();

    let (grandpa_protocol_config, grandpa_notification_service) =
        sc_consensus_grandpa::grandpa_peers_set_config::<Block, sc_network::NetworkWorker<Block, <Block as BlockT>::Hash>>(
            grandpa_protocol_name.clone(),
            metrics.clone(),
            peer_store_handle.clone(),
        );

    net_config.add_notification_protocol(grandpa_protocol_config);

    // ストレージノードGossipプロトコル設定
    let (storage_gossip_config, storage_notification_service) =
        crate::gossip::storage_nodes_peers_set_config::<Block, sc_network::NetworkWorker<Block, <Block as BlockT>::Hash>>(
            metrics.clone(),
            peer_store_handle,
        );

    net_config.add_notification_protocol(storage_gossip_config);

    let warp_sync = Arc::new(
        sc_consensus_grandpa::warp_proof::NetworkProvider::new(
            backend.clone(),
            grandpa_link.shared_authority_set().clone(),
            Vec::default(),
        ),
    );

    let (network, system_rpc_tx, tx_handler_controller, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            net_config,
            block_announce_validator_builder: None,
            warp_sync_config: Some(WarpSyncConfig::WithProvider(warp_sync)),
            block_relay: None,
            metrics,
        })?;

    if config.offchain_worker.enabled {
        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-worker",
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                is_validator: config.role.is_authority(),
                keystore: Some(keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: Arc::new(network.clone()),
                enable_http_requests: true,
                custom_extensions: |_| vec![],
            })?
            .run(client.clone(), task_manager.spawn_handle())
            .boxed(),
        );
    }

    let role = config.role;
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();

    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();
        // Storage Nodeレジストリを全接続で共有 (重要: closureの外で作成)
        // マルチノード対応：複数ノードを登録し、断片を分散配置
        let storage_nodes = crate::rpc::create_shared_storage_nodes();

        // ストレージノードGossipサービスをスポーン
        let (gossip_service, gossip_handle) = crate::gossip::StorageNodeGossip::new_with_handle(
            storage_nodes.clone(),
            storage_notification_service,
        );
        task_manager.spawn_handle().spawn(
            "storage-nodes-gossip",
            "storage-gossip",
            gossip_service.run(),
        );
        info!("Storage node gossip service spawned");

        // ストレージノード通信用のSr25519キーペアを起動時に生成
        // このキーペアでX-Chain-Authヘッダーを署名し、ストレージノード側で検証する
        // なりすましは許容（公開鍵のオンチェーン確認はしない）、ミス防止用の軽量認証
        let chain_keypair = {
            use sp_core::Pair;
            let (pair, _) = sp_core::sr25519::Pair::generate();
            let keypair = std::sync::Arc::new(pair);
            info!(
                "Generated Sr25519 keypair for X-Chain-Auth: {}",
                hex::encode(keypair.public().0)
            );
            keypair
        };

        Box::new(move |_| {
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: pool.clone(),
                storage_nodes: storage_nodes.clone(),
                gossip_handle: gossip_handle.clone(),
                chain_keypair: Some(chain_keypair.clone()),
            };
            crate::rpc::create_full(deps).map_err(Into::into)
        })
    };

    let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        network: network.clone(),
        client: client.clone(),
        keystore: keystore_container.keystore(),
        task_manager: &mut task_manager,
        transaction_pool: transaction_pool.clone(),
        rpc_builder: rpc_extensions_builder,
        backend,
        system_rpc_tx,
        tx_handler_controller,
        sync_service: sync_service.clone(),
        config,
        telemetry: telemetry.as_mut(),
        tracing_execute_block: None,
    })?;

    if mine {
        use parity_scale_codec::Encode as _;
        use sp_runtime::AccountId32;

        // --coinbase が未指定なら起動時にエラー
        let coinbase_str = coinbase
            .as_ref()
            .ok_or_else(|| ServiceError::Other("--coinbase is required when --mine is set".into()))?;

        // SS58 アドレスを AccountId32 にパース
        let coinbase_account: AccountId32 = coinbase_str
            .parse()
            .map_err(|e| ServiceError::Other(format!("invalid --coinbase SS58 address: {:?}", e)))?;

        // PreRuntime digest ペイロード = SCALE-encoded AccountId32 bytes
        // sc_consensus_pow::start_mining_worker が b"ANRC" engine-ID で自動ラップする
        let pre_runtime_bytes = coinbase_account.encode();

        let proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x: &Telemetry| x.handle()),
        );

        // PoW ブロック生成 (Phase B)
        // --randomx-mode に従い full / light dataset を選択する。
        let pow_algo = crate::pow::RandomXAlgorithm::new(client.clone(), randomx_mode.full_mode());

        let (_mining_handle, pow_worker) = sc_consensus_pow::start_mining_worker(
            Box::new(block_import),
            client.clone(),
            select_chain,
            pow_algo,
            proposer_factory,
            sync_service.clone(),
            sync_service.clone(),
            Some(pre_runtime_bytes), // coinbase AccountId32 を PreRuntime digest に設定
            move |_, ()| async move {
                let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
                Ok(timestamp)
            },
            Duration::from_secs(10),
            Duration::from_secs(10),
        );

        task_manager
            .spawn_essential_handle()
            .spawn_blocking("pow-miner", Some("block-authoring"), pow_worker);
    }

    if enable_grandpa {
        let grandpa_config = sc_consensus_grandpa::Config {
            gossip_duration: Duration::from_millis(333),
            justification_generation_period: 512,
            name: Some(name),
            observer_enabled: false,
            keystore: Some(keystore_container.keystore()),
            local_role: role,
            telemetry: telemetry.as_ref().map(|x: &Telemetry| x.handle()),
            protocol_name: grandpa_protocol_name,
        };

        let grandpa_voter =
            sc_consensus_grandpa::run_grandpa_voter(sc_consensus_grandpa::GrandpaParams {
                config: grandpa_config,
                link: grandpa_link,
                network,
                sync: sync_service,
                notification_service: grandpa_notification_service,
                voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
                prometheus_registry,
                shared_voter_state: SharedVoterState::empty(),
                telemetry: telemetry.as_ref().map(|x: &Telemetry| x.handle()),
                offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool),
            })?;

        task_manager
            .spawn_essential_handle()
            .spawn_blocking("grandpa-voter", None, grandpa_voter);
    }

    Ok(task_manager)
}
