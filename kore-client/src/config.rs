//

use network::Config as NetworkConfig;
use serde::Deserialize;

/// The Kore Client configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// The database path.
    pub db_path: String,
    /// The node identifier.
    pub node_id: String,
    /// The network configuration.
    pub network_config: NetworkConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use network::{NodeType, RoutingNode};

    #[test]
    fn test_config_creation() {
        let network_config = NetworkConfig {
            node_type: NodeType::Addressable,
            listen_addresses: vec!["/ip4/127.0.0.1/tcp/50000".to_string()],
            external_addresses: vec![],
            boot_nodes: vec![],
            tell: Default::default(),
            req_res: Default::default(),
            routing: Default::default(),
            control_list: Default::default(),
        };

        let config = Config {
            db_path: "/tmp/kore-client".to_string(),
            node_id: "JqA4bewRn5H1dRDFBsZ9e1udwk28BUtUSHBwQ_BJYASA".to_owned(),
            network_config: network_config.clone(),
        };

        assert_eq!(config.db_path, "/tmp/kore-client");
        assert_eq!(config.network_config.node_type, NodeType::Addressable);
        assert_eq!(config.network_config.listen_addresses.len(), 1);
    }

    #[test]
    fn test_config_with_multiple_addresses() {
        let network_config = NetworkConfig {
            node_type: NodeType::Bootstrap,
            listen_addresses: vec![
                "/ip4/127.0.0.1/tcp/50000".to_string(),
                "/ip4/0.0.0.0/tcp/50001".to_string(),
            ],
            external_addresses: vec!["/ip4/192.168.1.100/tcp/50000".to_string()],
            boot_nodes: vec![],
            tell: Default::default(),
            req_res: Default::default(),
            routing: Default::default(),
            control_list: Default::default(),
        };

        let config = Config {
            db_path: "/var/lib/kore".to_string(),
            node_id: "JqA4bewRn5H1dRDFBsZ9e1udwk28BUtUSHBwQ_BJYASA".to_owned(),
            network_config,
        };

        assert_eq!(config.network_config.listen_addresses.len(), 2);
        assert_eq!(config.network_config.external_addresses.len(), 1);
        assert_eq!(config.network_config.node_type, NodeType::Bootstrap);
    }

    #[test]
    fn test_config_with_boot_nodes() {
        let boot_node = RoutingNode {
            address: vec!["/ip4/127.0.0.1/tcp/40000".to_string()],
            peer_id: "12D3KooWRS3QVwqBtNp7rUCG4SF3nBrinQqJYC1N5qc1Wdr4jrze"
                .to_string(),
        };

        let network_config = NetworkConfig {
            node_type: NodeType::Ephemeral,
            listen_addresses: vec!["/ip4/127.0.0.1/tcp/50000".to_string()],
            external_addresses: vec![],
            boot_nodes: vec![boot_node],
            tell: Default::default(),
            req_res: Default::default(),
            routing: Default::default(),
            control_list: Default::default(),
        };

        let config = Config {
            db_path: "/tmp/ephemeral-node".to_string(),
            node_id: "JqA4bewRn5H1dRDFBsZ9e1udwk28BUtUSHBwQ_BJYASA".to_owned(),
            network_config,
        };

        assert_eq!(config.network_config.boot_nodes.len(), 1);
        assert_eq!(config.network_config.node_type, NodeType::Ephemeral);
    }

    #[test]
    fn test_config_clone() {
        let network_config = NetworkConfig {
            node_type: NodeType::Addressable,
            listen_addresses: vec!["/ip4/127.0.0.1/tcp/50000".to_string()],
            external_addresses: vec![],
            boot_nodes: vec![],
            tell: Default::default(),
            req_res: Default::default(),
            routing: Default::default(),
            control_list: Default::default(),
        };

        let config = Config {
            db_path: "/tmp/kore-client".to_string(),
            node_id: "JqA4bewRn5H1dRDFBsZ9e1udwk28BUtUSHBwQ_BJYASA".to_owned(),
            network_config,
        };

        let cloned_config = config.clone();

        assert_eq!(config.db_path, cloned_config.db_path);
        assert_eq!(
            config.network_config.node_type,
            cloned_config.network_config.node_type
        );
    }

    #[test]
    fn test_config_debug() {
        let network_config = NetworkConfig::default();
        let config = Config {
            db_path: "/tmp/test".to_string(),
            node_id: "JqA4bewRn5H1dRDFBsZ9e1udwk28BUtUSHBwQ_BJYASA".to_owned(),
            network_config,
        };

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("Config"));
        assert!(debug_str.contains("/tmp/test"));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{
            "db_path": "/tmp/kore",
            "node_id": "JqA4bewRn5H1dRDFBsZ9e1udwk28BUtUSHBwQ_BJYASA",
            "network_config": {
                "node_type": "Addressable",
                "listen_addresses": ["/ip4/127.0.0.1/tcp/50000"],
                "external_addresses": [],
                "boot_nodes": [],
                "tell": {
                    "message_timeout": {
                        "secs": 10,
                        "nanos": 0
                    },
                    "max_concurrent_streams": 100
                },
                "req_res": {
                    "message_timeout": {
                        "secs": 10,
                        "nanos": 0
                    },
                    "max_concurrent_streams": 100
                },
                "routing": {
                    "dht_random_walk": false,
                    "discovery_only_if_under_num": 18446744073709551615,
                    "allow_private_address_in_dht": false,
                    "allow_dns_address_in_dht": false,
                    "allow_loop_back_address_in_dht": false,
                    "kademlia_disjoint_query_paths": true
                },
                "control_list": {
                    "enable": false,
                    "allow_list": [],
                    "block_list": [],
                    "service_allow_list": [],
                    "service_block_list": [],
                    "interval_request": {
                        "secs": 0,
                        "nanos": 0
                    }
                }
            }
        }"#;

        let config: Result<Config, _> = serde_json::from_str(json);
        assert!(config.is_ok(), "Failed to deserialize config: {:?}", config.err());

        let config = config.unwrap();
        assert_eq!(config.db_path, "/tmp/kore");
        assert_eq!(config.network_config.node_type, NodeType::Addressable);
    }

    #[test]
    fn test_config_empty_db_path() {
        let network_config = NetworkConfig::default();
        let config = Config {
            db_path: String::new(),
            node_id: "JqA4bewRn5H1dRDFBsZ9e1udwk28BUtUSHBwQ_BJYASA".to_owned(),
            network_config,
        };

        assert!(config.db_path.is_empty());
    }

    #[test]
    fn test_config_various_node_types() {
        let node_types = vec![
            NodeType::Bootstrap,
            NodeType::Addressable,
            NodeType::Ephemeral,
        ];

        for node_type in node_types {
            let network_config = NetworkConfig {
                node_type: node_type.clone(),
                listen_addresses: vec!["/ip4/127.0.0.1/tcp/50000".to_string()],
                external_addresses: vec![],
                boot_nodes: vec![],
                tell: Default::default(),
                req_res: Default::default(),
                routing: Default::default(),
                control_list: Default::default(),
            };

            let config = Config {
                db_path: "/tmp/test".to_string(),
                node_id: "JqA4bewRn5H1dRDFBsZ9e1udwk28BUtUSHBwQ_BJYASA".to_owned(),
                network_config,
            };

            assert_eq!(config.network_config.node_type, node_type);
        }
    }
}