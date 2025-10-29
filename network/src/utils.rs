use crate::{Error, routing::RoutingNode};
use ip_network::IpNetwork;
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol, swarm::ConnectionId};
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use tracing::error;

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
    str::FromStr,
    time::Duration,
};

const TARGET_UTILS: &str = "KoreNetwork-Utils";

pub enum ScheduleType {
    Discover,
    Dial(Vec<Multiaddr>),
}

#[derive(Copy, Clone, Debug)]
pub enum Action {
    Discover,
    Dial,
    Identified(ConnectionId),
}

impl From<RetryKind> for Action {
    fn from(value: RetryKind) -> Self {
        match value {
            RetryKind::Discover => Action::Discover,
            RetryKind::Dial => Action::Dial,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum RetryKind {
    Discover,
    Dial,
}

#[derive(Clone, Debug)]
pub struct RetryState {
    pub attempts: u8,
    pub when: Instant,
    pub kind: RetryKind,
    pub addrs: Vec<Multiaddr>,
}

#[derive(Eq, Clone, Debug)]
pub struct Due(pub PeerId, pub Instant);
impl PartialEq for Due {
    fn eq(&self, o: &Self) -> bool {
        self.1.eq(&o.1)
    }
}
impl Ord for Due {
    fn cmp(&self, o: &Self) -> Ordering {
        o.1.cmp(&self.1)
    }
}
impl PartialOrd for Due {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Network state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NetworkState {
    /// Start.
    Start,
    /// Dial.
    Dial,
    /// Dialing boot node.
    Dialing,
    /// Running.
    Running,
    /// Disconnected.
    Disconnected,
}

/// Metric labels for the messages.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct MetricLabels {
    /// Fact.
    pub fact: Fact,
    /// Peer ID.
    pub peer_id: String,
}

/// Fact related to the message (sent or received).
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
pub enum Fact {
    /// Message sent.
    Sent,
    /// Message received.
    Received,
}

pub enum MessagesHelper {
    Single(Vec<u8>),
    Vec(VecDeque<Vec<u8>>),
}

/// Method that update allow and block lists
pub async fn request_update_lists(
    service_allow: &[String],
    service_block: &[String],
) -> ((Vec<String>, Vec<String>), (u16, u16)) {
    let mut vec_allow_peers: Vec<String> = vec![];
    let mut vec_block_peers: Vec<String> = vec![];
    let mut successful_allow: u16 = 0;
    let mut successful_block: u16 = 0;
    let client = reqwest::Client::new();

    for service in service_allow {
        match client.get(service).send().await {
            Ok(res) => {
                let fail = !res.status().is_success();
                if !fail {
                    match res.json().await {
                        Ok(peers) => {
                            let peers: Vec<String> = peers;
                            vec_allow_peers.append(&mut peers.clone());
                            successful_allow += 1;
                        }
                        Err(e) => {
                            error!(
                                TARGET_UTILS,
                                "Error performing Get {}, The server did not return what was expected: {}",
                                service,
                                e
                            );
                        }
                    }
                } else {
                    error!(
                        TARGET_UTILS,
                        "Error performing Get {}, The server did not return a correct code: {}",
                        service,
                        res.status()
                    );
                }
            }
            Err(e) => {
                error!(TARGET_UTILS, "Error performing Get {}: {}", service, e);
            }
        }
    }

    for service in service_block {
        match client.get(service).send().await {
            Ok(res) => {
                let fail = !res.status().is_success();
                if !fail {
                    match res.json().await {
                        Ok(peers) => {
                            let peers: Vec<String> = peers;
                            vec_block_peers.append(&mut peers.clone());
                            successful_block += 1;
                        }
                        Err(e) => {
                            error!(
                                TARGET_UTILS,
                                "Error performing Get {}, The server did not return what was expected: {}",
                                service,
                                e
                            );
                        }
                    }
                } else {
                    error!(
                        TARGET_UTILS,
                        "Error performing Get {}, The server did not return a correct code: {}",
                        service,
                        res.status()
                    );
                }
            }
            Err(e) => {
                error!(TARGET_UTILS, "Error performing Get {}: {}", service, e);
            }
        }
    }

    (
        (vec_allow_peers, vec_block_peers),
        (successful_allow, successful_block),
    )
}

/// Convert boot nodes to `PeerId` and `Multiaddr`.
pub fn convert_boot_nodes(
    boot_nodes: &[RoutingNode],
) -> HashMap<PeerId, Vec<Multiaddr>> {
    let mut boot_nodes_aux = HashMap::new();

    for node in boot_nodes {
        let Ok(peer) = bs58::decode(node.peer_id.clone()).into_vec() else {
            continue;
        };

        let Ok(peer) = PeerId::from_bytes(peer.as_slice()) else {
            continue;
        };

        let mut aux_addrs = vec![];
        for addr in node.address.iter() {
            let Ok(addr) = Multiaddr::from_str(addr) else {
                continue;
            };

            aux_addrs.push(addr);
        }

        if !aux_addrs.is_empty() {
            boot_nodes_aux.insert(peer, aux_addrs);
        }
    }

    boot_nodes_aux
}

/// Gets the list of external (public) addresses for the node from string array.
pub fn convert_addresses(
    addresses: &[String],
) -> Result<HashSet<Multiaddr>, Error> {
    let mut addrs = HashSet::new();
    for address in addresses {
        if let Some(value) = multiaddr(address) {
            addrs.insert(value);
        } else {
            return Err(Error::Address(format!(
                "Invalid MultiAddress conversion in External Address: {}",
                address
            )));
        }
    }
    Ok(addrs)
}

/// Parses a string into a `Multiaddr` if possible.
fn multiaddr(addr: &str) -> Option<Multiaddr> {
    addr.parse::<Multiaddr>().ok()
}

/// Check if the given `Multiaddr` is reachable.
///
/// This test is successful only for global IP addresses and DNS names.
// NB: Currently all DNS names are allowed and no check for TLD suffixes is done
// because the set of valid domains is highly dynamic and would require frequent
// updates, for example by utilising publicsuffix.org or IANA.
pub fn is_global(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| match p {
        Protocol::Ip4(ip) => IpNetwork::from(ip).is_global(),
        Protocol::Ip6(ip) => IpNetwork::from(ip).is_global(),
        _ => false,
    })
}

pub fn is_private(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| match p {
        Protocol::Ip4(ip) => ip.is_private(),
        Protocol::Ip6(ip) => ip.is_unique_local(),
        _ => false,
    })
}

pub fn is_loop_back(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| match p {
        Protocol::Ip4(ip) => ip.is_loopback(),
        Protocol::Ip6(ip) => ip.is_loopback(),
        _ => false,
    })
}

pub fn is_dns(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| {
        matches!(p, Protocol::Dns(_) | Protocol::Dns4(_) | Protocol::Dns6(_))
    })
}

/// Chech if the given `Multiaddr` is a memory address.
pub fn is_tcp(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| matches!(p, Protocol::Tcp(_)))
}

/// The configuration for a `Behaviour` protocol.
#[derive(Debug, Clone, Deserialize)]
pub struct ReqResConfig {
    /// message timeout
    pub message_timeout: Duration,
    /// max concurrent streams
    pub max_concurrent_streams: usize,
}

impl ReqResConfig {
    /// Create a ReqRes Confing
    pub fn new(
        message_timeout: Duration,
        max_concurrent_streams: usize,
    ) -> Self {
        Self {
            message_timeout,
            max_concurrent_streams,
        }
    }
}

impl Default for ReqResConfig {
    fn default() -> Self {
        Self {
            message_timeout: Duration::from_secs(10),
            max_concurrent_streams: 100,
        }
    }
}

impl ReqResConfig {
    /// Sets the timeout for inbound and outbound requests.
    pub fn with_message_timeout(mut self, timeout: Duration) -> Self {
        self.message_timeout = timeout;
        self
    }

    /// Sets the upper bound for the number of concurrent inbound + outbound streams.
    pub fn with_max_concurrent_streams(mut self, num_streams: usize) -> Self {
        self.max_concurrent_streams = num_streams;
        self
    }

    /// Get message timeout
    pub fn get_message_timeout(&self) -> Duration {
        self.message_timeout
    }

    /// Get max concurrent streams
    pub fn get_max_concurrent_streams(&self) -> usize {
        self.max_concurrent_streams
    }
}
