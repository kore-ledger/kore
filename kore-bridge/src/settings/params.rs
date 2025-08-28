// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::BTreeMap, time::Duration};

use identity::identifier::derive::{KeyDerivator, digest::DigestDerivator};
use kore_base::config::{ExternalDbConfig, KoreDbConfig};
use network::{NodeType, RoutingNode};
use serde::{Deserialize, Deserializer};
use tracing::error;

use crate::config::Config;

const TARGET_PARAMS: &str = "Kore-Bridge-Params";

#[derive(Debug, Deserialize, Default)]
pub struct Params {
    kore: KoreParams,
}

impl Params {
    pub fn from_env() -> Self {
        Self {
            kore: KoreParams::from_env("KORE"),
        }
    }

    pub fn mix_config(&self, other_config: Params) -> Self {
        Self {
            kore: self.kore.mix_config(other_config.kore),
        }
    }
}

impl From<Params> for Config {
    fn from(params: Params) -> Self {
        let tell = network::TellConfig::new(
            params.kore.network.tell.message_timeout_secs,
            params.kore.network.tell.max_concurrent_streams,
        );

        let req_res = network::ReqResConfig::new(
            params.kore.network.req_res.message_timeout_secs,
            params.kore.network.req_res.max_concurrent_streams,
        );

        let routing = network::RoutingConfig::new()
            .with_dht_random_walk(params.kore.network.routing.dht_random_walk)
            .with_discovery_limit(
                params.kore.network.routing.discovery_only_if_under_num,
            )
            .with_allow_local_address_in_dht(
                params.kore.network.routing.allow_local_address_in_dht,
            )
            .with_allow_loop_back_address_in_dht(
                params.kore.network.routing.allow_loop_back_address_in_dht,
            )
            .with_allow_dns_address_in_dht(
                params.kore.network.routing.allow_dns_address_in_dht,
            )
            .with_kademlia_disjoint_query_paths(
                params.kore.network.routing.kademlia_disjoint_query_paths,
            );

        let control_list = network::ControlListConfig::default()
            .with_allow_list(params.kore.network.control_list.allow_list)
            .with_block_list(params.kore.network.control_list.block_list)
            .with_enable(params.kore.network.control_list.enable)
            .with_interval_request(
                params.kore.network.control_list.interval_request,
            )
            .with_service_allow_list(
                params.kore.network.control_list.service_allow_list,
            )
            .with_service_block_list(
                params.kore.network.control_list.service_block_list,
            );

        Self {
            keys_path: params.kore.keys_path,
            prometheus: params.kore.prometheus,
            logging: kore_base::config::Logging {
                output: params.kore.logging.output,
                api_url: params.kore.logging.api_url,
                file_path: params.kore.logging.file_path,
                rotation: params.kore.logging.rotation,
                max_size: params.kore.logging.max_size,
                max_files: params.kore.logging.max_files,
                level: params.kore.logging.level,
            },
            kore_config: kore_base::config::Config {
                key_derivator: params.kore.base.key_derivator,
                digest_derivator: params.kore.base.digest_derivator,
                kore_db: params.kore.base.kore_db,
                external_db: params.kore.base.external_db,
                network: network::Config {
                    boot_nodes: params.kore.network.boot_nodes,
                    node_type: params.kore.network.node_type,
                    listen_addresses: params.kore.network.listen_addresses,
                    external_addresses: params.kore.network.external_addresses,
                    tell,
                    req_res,
                    routing,
                    control_list,
                },
                contracts_dir: params.kore.base.contracts_dir,
                always_accept: params.kore.base.always_accept,
                garbage_collector: params.kore.base.garbage_collector,
                sink: params.kore.base.sink,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct KoreParams {
    #[serde(default)]
    network: NetworkParams,
    #[serde(default)]
    base: BaseParams,
    #[serde(default = "default_keys_path")]
    keys_path: String,
    #[serde(default = "default_prometheus")]
    prometheus: String,
    #[serde(default)]
    logging: LoggingParams,
}

impl KoreParams {
    fn from_env(parent: &str) -> Self {
        let mut config = config::Config::builder();
        config = config.add_source(config::Environment::with_prefix(parent));

        let config = config
            .build()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error building config: {}", e);
            })
            .unwrap();

        let kore_params: KoreParams = config
            .try_deserialize()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error try deserialize config: {}", e);
            })
            .unwrap();

        Self {
            network: NetworkParams::from_env(&format!("{parent}_")),
            base: BaseParams::from_env(&format!("{parent}_")),
            keys_path: kore_params.keys_path,
            prometheus: kore_params.prometheus,
            logging: LoggingParams::from_env(&format!("{parent}_")),
        }
    }

    fn mix_config(&self, other_config: KoreParams) -> Self {
        let keys_path = if other_config.keys_path != default_keys_path() {
            other_config.keys_path
        } else {
            self.keys_path.clone()
        };

        let prometheus = if other_config.prometheus != default_prometheus() {
            other_config.prometheus
        } else {
            self.prometheus.clone()
        };
        Self {
            network: self.network.mix_config(other_config.network),
            base: self.base.mix_config(other_config.base),
            keys_path,
            prometheus,
            logging: self.logging.mix_config(other_config.logging),
        }
    }
}

impl Default for KoreParams {
    fn default() -> Self {
        Self {
            network: NetworkParams::default(),
            base: BaseParams::default(),
            keys_path: default_keys_path(),
            prometheus: default_prometheus(),
            logging: LoggingParams::default(),
        }
    }
}

fn default_prometheus() -> String {
    "0.0.0.0:3050".to_owned()
}

fn default_keys_path() -> String {
    "keys".to_owned()
}

#[derive(Debug, Deserialize, Clone)]
struct LoggingParams {
    #[serde(default = "default_log_output")]
    pub output: String, // "stdout(Docker)" | "file" | "api"
    #[serde(default)]
    pub api_url: Option<String>,
    #[serde(default = "default_log_file_path")]
    pub file_path: String, // ruta base de logs
    #[serde(default = "default_log_rotation")]
    pub rotation: String, // "size" | "hourly" | "daily"
    #[serde(default = "default_log_max_size")]
    pub max_size: u64, // bytes
    #[serde(default = "default_log_max_files")]
    pub max_files: usize, // copias a conservar
    #[serde(default = "default_log_level")]
    pub level: String, // "info", "debug", …
}

fn default_log_output() -> String {
    "stdout".into()
}
fn default_log_file_path() -> String {
    "logs/kore.log".into()
}
fn default_log_rotation() -> String {
    "size".into()
}
fn default_log_max_size() -> u64 {
    100 * 1024 * 1024
}
fn default_log_max_files() -> usize {
    3
}
fn default_log_level() -> String {
    "info".into()
}

impl Default for LoggingParams {
    fn default() -> Self {
        LoggingParams {
            output: default_log_output(),
            api_url: None,
            file_path: default_log_file_path(),
            rotation: default_log_rotation(),
            max_size: default_log_max_size(),
            max_files: default_log_max_files(),
            level: default_log_level(),
        }
    }
}

impl LoggingParams {
    /// Lee logging desde ENV vars con el prefijo (p. ej. "KORE_LOGGING_OUTPUT" etc).
    fn from_env(parent: &str) -> Self {
        let mut cfg = config::Config::builder();
        cfg = cfg.add_source(
            config::Environment::with_prefix(&format!("{parent}LOGGING"))
                .try_parsing(true),
        );
        let built = cfg.build().unwrap();
        built.try_deserialize().unwrap_or_default()
    }

    /// Combina self (prioridad) con other (fallback)
    fn mix_config(&self, other: LoggingParams) -> LoggingParams {
        LoggingParams {
            output: if self.output != default_log_output() {
                self.output.clone()
            } else {
                other.output
            },
            api_url: if self.api_url.is_some() {
                self.api_url.clone()
            } else {
                other.api_url
            },
            file_path: if self.file_path != default_log_file_path() {
                self.file_path.clone()
            } else {
                other.file_path
            },
            rotation: if self.rotation != default_log_rotation() {
                self.rotation.clone()
            } else {
                other.rotation
            },
            max_size: if self.max_size != default_log_max_size() {
                self.max_size
            } else {
                other.max_size
            },
            max_files: if self.max_files != default_log_max_files() {
                self.max_files
            } else {
                other.max_files
            },
            level: if self.level != default_log_level() {
                self.level.clone()
            } else {
                other.level
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct NetworkParams {
    #[serde(default = "default_user_agent")]
    user_agent: String,
    #[serde(default = "default_node_type")]
    node_type: NodeType,
    #[serde(default)]
    listen_addresses: Vec<String>,
    #[serde(default)]
    external_addresses: Vec<String>,
    #[serde(default)]
    tell: TellParams,
    #[serde(default)]
    req_res: ReqResParams,
    #[serde(default)]
    routing: RoutingParams,
    #[serde(default)]
    port_reuse: bool,
    #[serde(default)]
    control_list: ControlListParams,
    #[serde(default, deserialize_with = "deserialize_boot_nodes")]
    boot_nodes: Vec<RoutingNode>,
}

impl NetworkParams {
    fn from_env(parent: &str) -> Self {
        let mut config = config::Config::builder();
        config = config.add_source(
            config::Environment::with_prefix(&format!("{parent}NETWORK"))
                .list_separator(",")
                .with_list_parse_key("listen_addresses")
                .with_list_parse_key("boot_nodes")
                .with_list_parse_key("external_addresses")
                .try_parsing(true),
        );

        let config = config
            .build()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error building config: {}", e);
            })
            .unwrap();

        let network: NetworkParams = config
            .try_deserialize()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error try deserialize config: {}", e);
            })
            .unwrap();

        let parent = &format!("{parent}NETWORK_");
        Self {
            boot_nodes: network.boot_nodes,
            user_agent: network.user_agent,
            node_type: network.node_type,
            listen_addresses: network.listen_addresses,
            external_addresses: network.external_addresses,
            tell: TellParams::from_env(parent),
            req_res: ReqResParams::from_env(parent),
            routing: RoutingParams::from_env(parent),
            port_reuse: network.port_reuse,
            control_list: ControlListParams::from_env(parent),
        }
    }

    fn mix_config(&self, other_config: NetworkParams) -> Self {
        let boot_nodes = if !other_config.boot_nodes.is_empty() {
            other_config.boot_nodes
        } else {
            self.boot_nodes.clone()
        };
        let user_agent = if other_config.user_agent != default_user_agent() {
            other_config.user_agent
        } else {
            self.user_agent.clone()
        };

        let node_type = if other_config.node_type != default_node_type() {
            other_config.node_type
        } else {
            self.node_type.clone()
        };

        let listen_addresses = if !other_config.listen_addresses.is_empty() {
            other_config.listen_addresses
        } else {
            self.listen_addresses.clone()
        };

        let external_addresses = if !other_config.external_addresses.is_empty()
        {
            other_config.external_addresses
        } else {
            self.external_addresses.clone()
        };

        let port_reuse = if other_config.port_reuse {
            other_config.port_reuse
        } else {
            self.port_reuse
        };

        Self {
            boot_nodes,
            user_agent,
            node_type,
            listen_addresses,
            external_addresses,
            tell: self.tell.mix_config(other_config.tell),
            routing: self.routing.mix_config(other_config.routing),
            port_reuse,
            req_res: self.req_res.mix_config(other_config.req_res),
            control_list: self
                .control_list
                .mix_config(other_config.control_list),
        }
    }
}

fn default_user_agent() -> String {
    "kore-node".to_owned()
}

fn default_node_type() -> NodeType {
    NodeType::Bootstrap
}

impl Default for NetworkParams {
    fn default() -> Self {
        Self {
            boot_nodes: vec![],
            user_agent: default_user_agent(),
            node_type: default_node_type(),
            listen_addresses: vec![],
            external_addresses: vec![],
            tell: TellParams::default(),
            req_res: ReqResParams::default(),
            routing: RoutingParams::default(),
            port_reuse: false,
            control_list: ControlListParams::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ControlListParams {
    #[serde(default)]
    enable: bool,
    #[serde(default)]
    allow_list: Vec<String>,
    #[serde(default)]
    block_list: Vec<String>,
    #[serde(default)]
    service_allow_list: Vec<String>,
    #[serde(default)]
    service_block_list: Vec<String>,
    #[serde(
        default = "default_interval_request_secs",
        deserialize_with = "deserialize_duration_secs"
    )]
    interval_request: Duration,
}

impl Default for ControlListParams {
    fn default() -> Self {
        Self {
            allow_list: vec![],
            block_list: vec![],
            enable: false,
            interval_request: default_interval_request_secs(),
            service_allow_list: vec![],
            service_block_list: vec![],
        }
    }
}

fn default_interval_request_secs() -> Duration {
    Duration::from_secs(60)
}

impl ControlListParams {
    fn from_env(parent: &str) -> Self {
        let mut config = config::Config::builder();
        config = config.add_source(
            config::Environment::with_prefix(&format!("{parent}CONTROL_LIST"))
                .list_separator(",")
                .with_list_parse_key("allow_list")
                .try_parsing(true)
                .list_separator(",")
                .with_list_parse_key("block_list")
                .try_parsing(true)
                .list_separator(",")
                .with_list_parse_key("service_allow_list")
                .try_parsing(true)
                .list_separator(",")
                .with_list_parse_key("service_block_list")
                .try_parsing(true),
        );

        let config = config
            .build()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error building config: {}", e);
            })
            .unwrap();

        config
            .try_deserialize()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error try deserialize config: {}", e);
            })
            .unwrap()
    }

    fn mix_config(&self, other_config: ControlListParams) -> Self {
        let enable = if other_config.enable {
            true
        } else {
            self.enable
        };

        let allow_list = if !other_config.allow_list.is_empty() {
            other_config.allow_list
        } else {
            self.allow_list.clone()
        };

        let block_list = if !other_config.block_list.is_empty() {
            other_config.block_list
        } else {
            self.block_list.clone()
        };

        let service_allow_list = if !other_config.service_allow_list.is_empty()
        {
            other_config.service_allow_list
        } else {
            self.service_allow_list.clone()
        };

        let service_block_list = if !other_config.service_block_list.is_empty()
        {
            other_config.service_block_list
        } else {
            self.service_block_list.clone()
        };

        let interval_request = if other_config.interval_request
            != default_interval_request_secs()
        {
            other_config.interval_request
        } else {
            self.interval_request
        };

        Self {
            allow_list,
            block_list,
            enable,
            interval_request,
            service_allow_list,
            service_block_list,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TellParams {
    #[serde(
        default = "default_message_timeout_secs",
        deserialize_with = "deserialize_duration_secs"
    )]
    message_timeout_secs: Duration,
    #[serde(default = "default_max_concurrent_streams")]
    max_concurrent_streams: usize,
}

impl TellParams {
    fn from_env(parent: &str) -> Self {
        let mut config = config::Config::builder();
        config = config.add_source(config::Environment::with_prefix(&format!(
            "{parent}TELL"
        )));

        let config = config
            .build()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error building config: {}", e);
            })
            .unwrap();

        config
            .try_deserialize()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error try deserialize config: {}", e);
            })
            .unwrap()
    }

    fn mix_config(&self, other_config: TellParams) -> Self {
        let message_timeout_secs = if other_config.message_timeout_secs
            != default_message_timeout_secs()
        {
            other_config.message_timeout_secs
        } else {
            self.message_timeout_secs
        };

        let max_concurrent_streams = if other_config.max_concurrent_streams
            != default_max_concurrent_streams()
        {
            other_config.max_concurrent_streams
        } else {
            self.max_concurrent_streams
        };
        Self {
            message_timeout_secs,
            max_concurrent_streams,
        }
    }
}

impl Default for TellParams {
    fn default() -> Self {
        Self {
            message_timeout_secs: default_message_timeout_secs(),
            max_concurrent_streams: default_max_concurrent_streams(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ReqResParams {
    #[serde(
        default = "default_message_timeout_secs",
        deserialize_with = "deserialize_duration_secs"
    )]
    message_timeout_secs: Duration,
    #[serde(default = "default_max_concurrent_streams")]
    max_concurrent_streams: usize,
}

impl ReqResParams {
    fn from_env(parent: &str) -> Self {
        let mut config = config::Config::builder();
        config = config.add_source(config::Environment::with_prefix(&format!(
            "{parent}REQRES"
        )));

        let config = config
            .build()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error building config: {}", e);
            })
            .unwrap();

        config
            .try_deserialize()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error try deserialize config: {}", e);
            })
            .unwrap()
    }

    fn mix_config(&self, other_config: ReqResParams) -> Self {
        let message_timeout_secs = if other_config.message_timeout_secs
            != default_message_timeout_secs()
        {
            other_config.message_timeout_secs
        } else {
            self.message_timeout_secs
        };

        let max_concurrent_streams = if other_config.max_concurrent_streams
            != default_max_concurrent_streams()
        {
            other_config.max_concurrent_streams
        } else {
            self.max_concurrent_streams
        };
        Self {
            message_timeout_secs,
            max_concurrent_streams,
        }
    }
}

impl Default for ReqResParams {
    fn default() -> Self {
        Self {
            message_timeout_secs: default_message_timeout_secs(),
            max_concurrent_streams: default_max_concurrent_streams(),
        }
    }
}

fn deserialize_duration_secs<'de, D>(
    deserializer: D,
) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let u: u64 = u64::deserialize(deserializer)?;
    Ok(Duration::from_secs(u))
}

fn default_max_concurrent_streams() -> usize {
    100
}

fn default_message_timeout_secs() -> Duration {
    Duration::from_secs(10)
}

#[derive(Debug, Deserialize)]
struct RoutingParams {
    #[serde(default = "default_true")]
    dht_random_walk: bool,
    #[serde(default = "default_discovery_only_if_under_num")]
    discovery_only_if_under_num: u64,
    #[serde(default)]
    allow_local_address_in_dht: bool,
    #[serde(default)]
    allow_dns_address_in_dht: bool,
    #[serde(default)]
    allow_loop_back_address_in_dht: bool,
    #[serde(default = "default_true")]
    kademlia_disjoint_query_paths: bool,
    #[serde(default)]
    kademlia_replication_factor: usize,
}

impl RoutingParams {
    fn from_env(parent: &str) -> Self {
        let mut config = config::Config::builder();
        config = config.add_source(
            config::Environment::with_prefix(&format!("{parent}ROUTING"))
                .list_separator(",")
                .with_list_parse_key("protocol_names")
                .try_parsing(true),
        );

        let config = config
            .build()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error building config: {}", e);
            })
            .unwrap();

        config
            .try_deserialize()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error try deserialize config: {}", e);
            })
            .unwrap()
    }

    fn mix_config(&self, other_config: RoutingParams) -> Self {
        let dht_random_walk = if !other_config.dht_random_walk {
            other_config.dht_random_walk
        } else {
            self.dht_random_walk
        };
        let discovery_only_if_under_num = if other_config
            .discovery_only_if_under_num
            != default_discovery_only_if_under_num()
        {
            other_config.discovery_only_if_under_num
        } else {
            self.discovery_only_if_under_num
        };

        let allow_local_address_in_dht =
            if other_config.allow_local_address_in_dht {
                other_config.allow_local_address_in_dht
            } else {
                self.allow_local_address_in_dht
            };

        let allow_dns_address_in_dht = if other_config.allow_dns_address_in_dht
        {
            other_config.allow_dns_address_in_dht
        } else {
            self.allow_dns_address_in_dht
        };

        let allow_loop_back_address_in_dht =
            if other_config.allow_loop_back_address_in_dht {
                other_config.allow_loop_back_address_in_dht
            } else {
                self.allow_loop_back_address_in_dht
            };

        let kademlia_disjoint_query_paths =
            if !other_config.kademlia_disjoint_query_paths {
                other_config.kademlia_disjoint_query_paths
            } else {
                self.kademlia_disjoint_query_paths
            };
        let kademlia_replication_factor =
            if other_config.kademlia_replication_factor != 0 {
                other_config.kademlia_replication_factor
            } else {
                self.kademlia_replication_factor
            };

        Self {
            dht_random_walk,
            discovery_only_if_under_num,
            allow_local_address_in_dht,
            allow_dns_address_in_dht,
            allow_loop_back_address_in_dht,
            kademlia_disjoint_query_paths,
            kademlia_replication_factor,
        }
    }
}

impl Default for RoutingParams {
    fn default() -> Self {
        Self {
            dht_random_walk: default_true(),
            discovery_only_if_under_num: default_discovery_only_if_under_num(),
            allow_local_address_in_dht: false,
            allow_dns_address_in_dht: false,
            allow_loop_back_address_in_dht: false,
            kademlia_disjoint_query_paths: default_true(),
            kademlia_replication_factor: 0,
        }
    }
}

fn deserialize_boot_nodes<'de, D>(
    deserializer: D,
) -> Result<Vec<RoutingNode>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Vec<String> = Vec::deserialize(deserializer)?;

    Ok(v.into_iter()
        .map(|element| {
            if let Some(pos) = element.find("/p2p/") {
                // La parte antes de "/p2p/" (no incluye "/p2p/")
                let address = &element[..pos].to_owned();
                // La parte después de "/p2p/"
                let peer_id = &element[pos + 5..].to_owned();
                RoutingNode {
                    address: address.split('_').map(|e| e.to_owned()).collect(),
                    peer_id: peer_id.clone(),
                }
            } else {
                RoutingNode {
                    address: vec![],
                    peer_id: String::default(),
                }
            }
        })
        .collect())
}

fn default_true() -> bool {
    true
}

fn default_discovery_only_if_under_num() -> u64 {
    u64::MAX
}

#[derive(Debug, Deserialize)]
struct BaseParams {
    #[serde(default = "default_key_derivator")]
    key_derivator: KeyDerivator,
    #[serde(default = "default_digest_derivator")]
    digest_derivator: DigestDerivator,
    #[serde(default)]
    always_accept: bool,
    #[serde(default = "default_contracts_directory")]
    contracts_dir: String,
    #[serde(
        default = "default_garbage_collector_secs",
        deserialize_with = "deserialize_duration_secs"
    )]
    garbage_collector: Duration,
    #[serde(default, deserialize_with = "KoreDbConfig::deserialize_db")]
    kore_db: KoreDbConfig,
    #[serde(default, deserialize_with = "ExternalDbConfig::deserialize_db")]
    external_db: ExternalDbConfig,
    #[serde(default, deserialize_with = "deserialize_sinks")]
    sink: BTreeMap<String, String>,
}

impl BaseParams {
    fn from_env(parent: &str) -> Self {
        let mut config = config::Config::builder();
        config = config.add_source(config::Environment::with_prefix(&format!(
            "{parent}BASE"
        )));

        let config = config
            .build()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error building config: {}", e);
            })
            .unwrap();

        config
            .try_deserialize()
            .map_err(|e| {
                error!(TARGET_PARAMS, "Error try deserialize config: {}", e);
            })
            .unwrap()
    }

    fn mix_config(&self, other_config: BaseParams) -> Self {
        let key_derivator =
            if other_config.key_derivator != default_key_derivator() {
                other_config.key_derivator
            } else {
                self.key_derivator
            };

        let digest_derivator =
            if other_config.digest_derivator != default_digest_derivator() {
                other_config.digest_derivator
            } else {
                self.digest_derivator
            };

        let always_accept = if other_config.always_accept {
            other_config.always_accept
        } else {
            self.always_accept
        };

        let contracts_dir =
            if other_config.contracts_dir != default_contracts_directory() {
                other_config.contracts_dir
            } else {
                self.contracts_dir.clone()
            };

        let garbage_collector = if other_config.garbage_collector
            != default_garbage_collector_secs()
        {
            other_config.garbage_collector
        } else {
            self.garbage_collector
        };

        let external_db =
            if other_config.external_db != ExternalDbConfig::default() {
                other_config.external_db
            } else {
                self.external_db.clone()
            };

        let kore_db = if other_config.kore_db != KoreDbConfig::default() {
            other_config.kore_db
        } else {
            self.kore_db.clone()
        };

        let sink = if !other_config.sink.is_empty() {
            other_config.sink
        } else {
            self.sink.clone()
        };

        Self {
            key_derivator,
            digest_derivator,
            always_accept,
            contracts_dir,
            garbage_collector,
            external_db,
            kore_db,
            sink,
        }
    }
}

impl Default for BaseParams {
    fn default() -> Self {
        Self {
            key_derivator: default_key_derivator(),
            digest_derivator: default_digest_derivator(),
            always_accept: Default::default(),
            contracts_dir: default_contracts_directory(),
            garbage_collector: default_garbage_collector_secs(),
            kore_db: Default::default(),
            external_db: Default::default(),
            sink: Default::default(),
        }
    }
}

fn deserialize_sinks<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let u: String = String::deserialize(deserializer)?;
    let elements = u.split(",").collect::<Vec<&str>>();
    let vec = elements
        .iter()
        .map(|x| match x.split_once(":") {
            Some((k, v)) => (k.to_owned(), v.to_owned()),
            None => (String::default(), String::default()),
        })
        .collect::<Vec<(String, String)>>();

    let mut btreemap = BTreeMap::from_iter(vec.iter().cloned());
    btreemap.remove("");

    Ok(btreemap)
}

fn default_garbage_collector_secs() -> Duration {
    Duration::from_secs(100)
}

fn default_contracts_directory() -> String {
    "./".to_owned()
}

fn default_key_derivator() -> KeyDerivator {
    KeyDerivator::Ed25519
}

fn default_digest_derivator() -> DigestDerivator {
    DigestDerivator::Blake3_256
}
