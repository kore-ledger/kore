// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;

use config::Config;
use kore_base::error::Error;
use params::Params;
use tracing::error;

pub mod command;
use crate::config::Config as BridgeConfig;
pub mod params;

const TARGET_SETTING: &str = "Kore-Bridge-Settings";

pub fn build_config(env: bool, file: &str) -> Result<BridgeConfig, Error> {
    // Env configuration
    let mut params_env = Params::default();
    if env {
        params_env = Params::from_env();
    }

    // file configuration (json, yaml or toml)
    let mut params_file = Params::default();
    if !file.is_empty() {
        let mut config = Config::builder();

        config = config.add_source(config::File::with_name(file));

        let config = config.build().map_err(|e| {
            let e = format!("Error building config: {}", e);
            error!(TARGET_SETTING, e);
            Error::Bridge(e)
        })?;

        params_file = config.try_deserialize().map_err(|e| {
            let e = format!("Error try deserialize config: {}", e);
            error!(TARGET_SETTING, e);
            Error::Bridge(e)
        })?;
    }

    // Mix configurations.
    Ok(BridgeConfig::from(params_env.mix_config(params_file)))
}

pub fn build_password() -> String {
    env::var("KORE_PASSWORD").unwrap_or("kore".to_owned())
}

pub fn build_file_path() -> String {
    env::var("KORE_FILE_PATH").unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use identity::identifier::derive::{KeyDerivator, digest::DigestDerivator};
    use network::{NodeType, RoutingNode};
    use serial_test::serial;

    use crate::settings::build_config;

    #[test]
    #[serial]
    fn test_env_full() {
        unsafe {
            std::env::set_var("KORE_NETWORK_TELL_MESSAGE_TIMEOUT_SECS", "58");
            std::env::set_var(
                "KORE_NETWORK_TELL_MAX_CONCURRENT_STREAMS",
                "166",
            );
            std::env::set_var("KORE_NETWORK_REQRES_MESSAGE_TIMEOUT_SECS", "59");
            std::env::set_var(
                "KORE_NETWORK_REQRES_MAX_CONCURRENT_STREAMS",
                "167",
            );
            std::env::set_var(
                "KORE_NETWORK_BOOT_NODES",
                "/ip4/172.17.0.1/tcp/50000_/ip4/127.0.0.1/tcp/60001/p2p/12D3KooWLXexpg81PjdjnrhmHUxN7U5EtfXJgr9cahei1SJ9Ub3B,/ip4/11.11.0.11/tcp/10000_/ip4/12.22.33.44/tcp/55511/p2p/12D3KooWRS3QVwqBtNp7rUCG4SF3nBrinQqJYC1N5qc1Wdr4jrze",
            );
            std::env::set_var("KORE_NETWORK_ROUTING_DHT_RANDOM_WALK", "false");
            std::env::set_var(
                "KORE_NETWORK_ROUTING_DISCOVERY_ONLY_IF_UNDER_NUM",
                "55",
            );
            std::env::set_var(
                "KORE_NETWORK_ROUTING_ALLOW_NON_GLOBALS_ADDRESS_IN_DHT",
                "true",
            );
            std::env::set_var(
                "KORE_NETWORK_ROUTING_KADEMLIA_DISJOINT_QUERY_PATHS",
                "false",
            );

            std::env::set_var("KORE_BASE_KEY_DERIVATOR", "Secp256k1");
            std::env::set_var("KORE_BASE_DIGEST_DERIVATOR", "Blake3_512");
            std::env::set_var("KORE_BASE_ALWAYS_ACCEPT", "true");
            std::env::set_var("KORE_BASE_CONTRACTS_DIR", "./fake_route");
            std::env::set_var("KORE_BASE_KORE_DB", "./fake/db/path");
            std::env::set_var("KORE_BASE_EXTERNAL_DB", "./fake/db/path");
            std::env::set_var("KORE_BASE_GARBAGE_COLLECTOR", "1000");
            std::env::set_var(
                "KORE_BASE_SINK",
                "key1:https://www.kore-ledger.net/build/,key2:https://www.kore-ledger.net/community/",
            );

            std::env::set_var("KORE_NETWORK_NODE_TYPE", "Addressable");
            std::env::set_var(
                "KORE_NETWORK_LISTEN_ADDRESSES",
                "/ip4/127.0.0.1/tcp/50000,/ip4/127.0.0.1/tcp/50001,/ip4/127.0.0.1/tcp/50002",
            );
            std::env::set_var(
                "KORE_NETWORK_EXTERNAL_ADDRESSES",
                "/ip4/90.1.0.60/tcp/50000,/ip4/90.1.0.61/tcp/50000",
            );

            std::env::set_var("KORE_KEYS_PATH", "./fake/keys/path");
            std::env::set_var("KORE_PROMETHEUS", "10.0.0.0:3030");

            std::env::set_var("KORE_NETWORK_CONTROL_LIST_ENABLE", "true");
            std::env::set_var(
                "KORE_NETWORK_CONTROL_LIST_ALLOW_LIST",
                "Peer200,Peer300",
            );
            std::env::set_var(
                "KORE_NETWORK_CONTROL_LIST_BLOCK_LIST",
                "Peer1,Peer2",
            );
            std::env::set_var(
                "KORE_NETWORK_CONTROL_LIST_SERVICE_ALLOW_LIST",
                "http://90.0.0.1:3000/allow_list,http://90.0.0.2:4000/allow_list",
            );
            std::env::set_var(
                "KORE_NETWORK_CONTROL_LIST_SERVICE_BLOCK_LIST",
                "http://90.0.0.1:3000/block_list,http://90.0.0.2:4000/block_list",
            );
            std::env::set_var(
                "KORE_NETWORK_CONTROL_LIST_INTERVAL_REQUEST",
                "58",
            );
            std::env::set_var("KORE_LOGGING_OUTPUT", "file");
            std::env::set_var("KORE_LOGGING_API_URL", "https://example.com/logs");
            std::env::set_var("KORE_LOGGING_FILE_PATH", "/tmp/my.log");
            std::env::set_var("KORE_LOGGING_ROTATION", "size");
            std::env::set_var("KORE_LOGGING_MAX_SIZE", "52428800");
            std::env::set_var("KORE_LOGGING_MAX_FILES", "5");
            std::env::set_var("KORE_LOGGING_LEVEL", "debug");
        }

        let config = build_config(true, "").unwrap();

        let boot_nodes = vec![
            RoutingNode {
                address: vec![
                    "/ip4/172.17.0.1/tcp/50000".to_owned(),
                    "/ip4/127.0.0.1/tcp/60001".to_owned(),
                ],
                peer_id: "12D3KooWLXexpg81PjdjnrhmHUxN7U5EtfXJgr9cahei1SJ9Ub3B"
                    .to_owned(),
            },
            RoutingNode {
                address: vec![
                    "/ip4/11.11.0.11/tcp/10000".to_owned(),
                    "/ip4/12.22.33.44/tcp/55511".to_owned(),
                ],
                peer_id: "12D3KooWRS3QVwqBtNp7rUCG4SF3nBrinQqJYC1N5qc1Wdr4jrze"
                    .to_owned(),
            },
        ];
        let log = &config.logging;
        assert_eq!(log.output, "file");
        assert_eq!(log.api_url.as_deref(), Some("https://example.com/logs"));
        assert_eq!(log.file_path, "/tmp/my.log");
        assert_eq!(log.rotation, "size");
        assert_eq!(log.max_size, 50 * 1024 * 1024);
        assert_eq!(log.max_files, 5);
        assert_eq!(log.level, "debug");

        assert_eq!(config.kore_config.network.node_type, NodeType::Addressable);
        assert_eq!(
            config.kore_config.network.listen_addresses,
            vec![
                "/ip4/127.0.0.1/tcp/50000".to_owned(),
                "/ip4/127.0.0.1/tcp/50001".to_owned(),
                "/ip4/127.0.0.1/tcp/50002".to_owned()
            ]
        );
        assert_eq!(
            config.kore_config.network.external_addresses,
            vec![
                "/ip4/90.1.0.60/tcp/50000".to_owned(),
                "/ip4/90.1.0.61/tcp/50000".to_owned(),
            ]
        );
        assert_eq!(config.kore_config.key_derivator, KeyDerivator::Secp256k1);
        assert_eq!(
            config.kore_config.digest_derivator,
            DigestDerivator::Blake3_512
        );
        assert_eq!(config.kore_config.always_accept, true);
        assert_eq!(
            config.kore_config.garbage_collector,
            Duration::from_secs(1000)
        );
        assert_eq!(config.kore_config.contracts_dir, "./fake_route");
        assert_eq!(config.kore_config.sink.len(), 2);

        assert_eq!(
            config.kore_config.network.boot_nodes[0].peer_id,
            boot_nodes[0].peer_id
        );
        assert_eq!(
            config.kore_config.network.boot_nodes[0].address,
            boot_nodes[0].address
        );
        assert_eq!(
            config.kore_config.network.boot_nodes[1].peer_id,
            boot_nodes[1].peer_id
        );
        assert_eq!(
            config.kore_config.network.boot_nodes[1].address,
            boot_nodes[1].address
        );

        assert_eq!(
            config.kore_config.network.routing.get_dht_random_walk(),
            false
        );
        assert_eq!(
            config.kore_config.network.routing.get_discovery_limit(),
            55
        );
        assert_eq!(
            config
                .kore_config
                .network
                .routing
                .get_allow_non_globals_address_in_dht(),
            true
        );
        assert_eq!(
            config
                .kore_config
                .network
                .routing
                .get_kademlia_disjoint_query_paths(),
            false
        );
        assert_eq!(
            config.kore_config.network.tell.get_message_timeout(),
            Duration::from_secs(58)
        );

        assert_eq!(
            config.kore_config.network.req_res.get_message_timeout(),
            Duration::from_secs(59)
        );
        assert_eq!(
            config.kore_config.network.tell.get_max_concurrent_streams(),
            166
        );
        assert_eq!(
            config.kore_config.network.req_res.get_max_concurrent_streams(),
            167
        );

        assert_eq!(config.keys_path, "./fake/keys/path".to_owned());
        assert_eq!(config.prometheus, "10.0.0.0:3030".to_owned());

        assert_eq!(
            config.kore_config.network.control_list.get_allow_list(),
            vec!["Peer200", "Peer300"]
        );
        assert_eq!(
            config.kore_config.network.control_list.get_block_list(),
            vec!["Peer1", "Peer2"]
        );
        assert_eq!(
            config
                .kore_config
                .network
                .control_list
                .get_service_allow_list(),
            vec![
                "http://90.0.0.1:3000/allow_list",
                "http://90.0.0.2:4000/allow_list"
            ]
        );
        assert_eq!(
            config
                .kore_config
                .network
                .control_list
                .get_service_block_list(),
            vec![
                "http://90.0.0.1:3000/block_list",
                "http://90.0.0.2:4000/block_list"
            ]
        );
        assert!(config.kore_config.network.control_list.get_enable());
        assert_eq!(
            config
                .kore_config
                .network
                .control_list
                .get_interval_request(),
            Duration::from_secs(58)
        );

        unsafe {
            std::env::remove_var("KORE_NETWORK_TELL_MESSAGE_TIMEOUT_SECS");
            std::env::remove_var("KORE_NETWORK_TELL_MAX_CONCURRENT_STREAMS");
            std::env::remove_var("KORE_NETWORK_REQRES_MESSAGE_TIMEOUT_SECS");
            std::env::remove_var("KORE_NETWORK_REQRES_MAX_CONCURRENT_STREAMS");
            std::env::remove_var("KORE_NETWORK_BOOT_NODES");
            std::env::remove_var("KORE_NETWORK_ROUTING_DHT_RANDOM_WALK");
            std::env::remove_var(
                "KORE_NETWORK_ROUTING_DISCOVERY_ONLY_IF_UNDER_NUM",
            );
            std::env::remove_var(
                "KORE_NETWORK_ROUTING_ALLOW_NON_GLOBALS_ADDRESS_IN_DHT",
            );
            std::env::remove_var(
                "KORE_NETWORK_ROUTING_KADEMLIA_DISJOINT_QUERY_PATHS",
            );
            std::env::remove_var("KORE_KEYS_PATH");
            std::env::remove_var("KORE_NETWORK_NODE_TYPE");
            std::env::remove_var("KORE_NETWORK_LISTEN_ADDRESSES");
            std::env::remove_var("KORE_NETWORK_EXTERNAL_ADDRESSES");
            std::env::remove_var("KORE_BASE_KEY_DERIVATOR");
            std::env::remove_var("KORE_BASE_DIGEST_DERIVATOR");
            std::env::remove_var("KORE_BASE_ALWAYS_ACCEPT");
            std::env::remove_var("KORE_BASE_CONTRACTS_DIR");
            std::env::remove_var("KORE_BASE_KORE_DB");
            std::env::remove_var("KORE_BASE_EXTERNAL_DB");
            std::env::remove_var("KORE_BASE_GARBAGE_COLLECTOR");
            std::env::remove_var("KORE_BASE_SINK");
            std::env::remove_var("KORE_PROMETHEUS");
            std::env::remove_var("KORE_NETWORK_CONTROL_LIST_ENABLE");
            std::env::remove_var("KORE_NETWORK_CONTROL_LIST_ALLOW_LIST");
            std::env::remove_var("KORE_NETWORK_CONTROL_LIST_BLOCK_LIST");
            std::env::remove_var(
                "KORE_NETWORK_CONTROL_LIST_SERVICE_ALLOW_LIST",
            );
            std::env::remove_var(
                "KORE_NETWORK_CONTROL_LIST_SERVICE_BLOCK_LIST",
            );
            std::env::remove_var("KORE_NETWORK_CONTROL_LIST_INTERVAL_REQUEST");
            std::env::remove_var("KORE_LOGGING_OUTPUT");
            std::env::remove_var("KORE_LOGGING_API_URL");
            std::env::remove_var("KORE_LOGGING_FILE_PATH");
            std::env::remove_var("KORE_LOGGING_ROTATION");
            std::env::remove_var("KORE_LOGGING_MAX_SIZE");
            std::env::remove_var("KORE_LOGGING_MAX_FILES");
            std::env::remove_var("KORE_LOGGING_LEVEL");
        }
    }
}
