// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use futures_timer::Delay;

use libp2p::{
    Multiaddr, PeerId, StreamProtocol,
    core::Endpoint,
    futures::FutureExt,
    kad::{
        Behaviour as Kademlia, BootstrapError, Config as KademliaConfig,
        Event as KademliaEvent, GetClosestPeersError, GetClosestPeersOk,
        PeerInfo, QueryResult, store::MemoryStore,
    },
    swarm::{
        CloseConnection, ConnectionDenied, ConnectionId, FromSwarm,
        NetworkBehaviour, THandler, ToSwarm,
    },
};
use serde::Deserialize;

use std::{
    cmp,
    collections::{HashMap, VecDeque},
    task::{Poll, Waker},
    time::Duration,
};

use crate::{NodeType, utils::is_reachable};

#[cfg(not(feature = "test"))]
use crate::utils::is_memory;

/// The discovery behaviour.
pub struct Behaviour {
    /// Boolean that activates the random walk if the node has already finished the initial pre-routing phase.
    pre_routing: bool,

    /// Kademlia behavior.
    kademlia: Kademlia<MemoryStore>,

    waker: Option<Waker>,

    /// The next random walk in the Kademlia DHT. `None` if random walks are disabled.
    next_random_walk: Option<Delay>,

    /// Duration between random walks.
    duration_to_next_kad: Duration,

    /// Number of nodes we're currently connected to.
    num_connections: u64,

    /// Number of active connections over which we interrupt the discovery process.
    discovery_only_if_under_num: u64,

    /// Whether to allow non-global addresses in the DHT.
    allow_non_globals_address_in_dht: bool,

    /// Peers to close connection
    close_connections: VecDeque<(PeerId, Option<ConnectionId>)>,

    peer_to_remove: HashMap<PeerId, u8>,
}

impl Behaviour {
    /// Creates a new routing `Behaviour`.
    pub fn new(
        peer_id: PeerId,
        config: Config,
        protocol: StreamProtocol,
        node_type: NodeType,
    ) -> Self {
        let Config {
            dht_random_walk,
            discovery_only_if_under_num,
            allow_non_globals_address_in_dht,
            kademlia_disjoint_query_paths,
        } = config;

        let mut kad_config = KademliaConfig::new(protocol);

        kad_config.disjoint_query_paths(kademlia_disjoint_query_paths);

        // By default Kademlia attempts to insert all peers into its routing table once a
        // dialing attempt succeeds. In order to control which peer is added, disable the
        // auto-insertion and instead add peers manually.
        kad_config.set_kbucket_inserts(libp2p::kad::BucketInserts::Manual);

        let store = MemoryStore::new(peer_id);
        let mut kad = Kademlia::with_config(peer_id, store, kad_config);

        if let NodeType::Addressable | NodeType::Bootstrap = node_type {
            kad.set_mode(Some(libp2p::kad::Mode::Server));
        } else {
            kad.set_mode(Some(libp2p::kad::Mode::Client));
        }

        Self {
            kademlia: kad,
            next_random_walk: if dht_random_walk {
                Some(Delay::new(Duration::new(0, 0)))
            } else {
                None
            },
            duration_to_next_kad: Duration::from_secs(1),
            num_connections: 0,
            discovery_only_if_under_num,
            allow_non_globals_address_in_dht,
            close_connections: VecDeque::new(),
            pre_routing: true,
            waker: None,
            peer_to_remove: HashMap::new(),
        }
    }

    pub fn new_close_connections(
        &mut self,
        peer_id: PeerId,
        connection_id: Option<ConnectionId>,
    ) {
        self.close_connections.push_back((peer_id, connection_id));

        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    pub fn finish_prerouting_state(&mut self) {
        self.pre_routing = false;
    }

    /// Returns true if the given peer is known.
    pub fn is_known_peer(&mut self, peer_id: &PeerId) -> bool {
        for b in self.kademlia.kbuckets() {
            if b.iter().any(|x| peer_id == x.node.key.preimage()) {
                return true;
            }
        }

        false
    }

    /// Add a self-reported address of a remote peer to the k-buckets of the DHT
    /// if it has compatible `supported_protocols`.
    ///
    /// **Note**: It is important that you call this method. The discovery mechanism will not
    /// automatically add connecting peers to the Kademlia k-buckets.
    pub fn add_self_reported_address(
        &mut self,
        peer_id: &PeerId,
        addr: Multiaddr,
    ) {
        #[cfg(not(feature = "test"))]
        {
            if is_memory(&addr) {
                return;
            }
        }

        if !self.allow_non_globals_address_in_dht && !is_reachable(&addr) {
            return;
        }

        self.kademlia.add_address(peer_id, addr.clone());
    }

    /// Discover closet peers to the given `PeerId`.
    pub fn discover(&mut self, peer_id: &PeerId) {
        self.kademlia.get_closest_peers(*peer_id);
    }

    /// Remove node from the DHT.
    pub fn remove_node(&mut self, peer_id: &PeerId) {
        self.kademlia.remove_peer(peer_id);
    }
}

/// Event generated by the `DiscoveryBehaviour`.
#[derive(Debug)]
pub enum Event {
    /// Closest peers to a given key have been found
    ClosestPeer {
        peer_id: PeerId,
        info: Option<PeerInfo>,
    },
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler =
        <Kademlia<MemoryStore> as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = Event;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.kademlia.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: libp2p::core::transport::PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.kademlia.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::AddressChange(..)
            | FromSwarm::DialFailure(..)
            | FromSwarm::ExpiredListenAddr(..)
            | FromSwarm::ExternalAddrConfirmed(..)
            | FromSwarm::ExternalAddrExpired(..)
            | FromSwarm::ListenerClosed(..)
            | FromSwarm::ListenFailure(..)
            | FromSwarm::ListenerError(..)
            | FromSwarm::NewListener(..)
            | FromSwarm::NewListenAddr(..)
            | FromSwarm::NewExternalAddrCandidate(..)
            | FromSwarm::NewExternalAddrOfPeer(..) => {
                self.kademlia.on_swarm_event(event);
            }
            FromSwarm::ConnectionEstablished(e) => {
                self.num_connections += 1;
                self.kademlia
                    .on_swarm_event(FromSwarm::ConnectionEstablished(e));
            }
            FromSwarm::ConnectionClosed(e) => {
                self.num_connections -= 1;
                self.kademlia.on_swarm_event(FromSwarm::ConnectionClosed(e));
            }
            _ => self.kademlia.on_swarm_event(event),
        }
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: libp2p::swarm::ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        self.kademlia.on_connection_handler_event(
            peer_id,
            connection_id,
            event,
        );
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<
        libp2p::swarm::ToSwarm<
            Self::ToSwarm,
            libp2p::swarm::THandlerInEvent<Self>,
        >,
    > {
        if let Some((peer_id, connection_id)) =
            self.close_connections.pop_front()
        {
            if let Some(connection_id) = connection_id {
                return Poll::Ready(ToSwarm::CloseConnection {
                    peer_id,
                    connection: CloseConnection::One(connection_id),
                });
            } else {
                return Poll::Ready(ToSwarm::CloseConnection {
                    peer_id,
                    connection: CloseConnection::All,
                });
            }
        }

        while let Poll::Ready(ev) = self.kademlia.poll(cx) {
            match ev {
                ToSwarm::GenerateEvent(ev) => {
                    match ev {
                        KademliaEvent::RoutablePeer { peer, address } => {
                            self.add_self_reported_address(&peer, address);
                        }
                        KademliaEvent::ModeChanged { .. }
                        | KademliaEvent::InboundRequest { .. }
                        | KademliaEvent::UnroutablePeer { .. }
                        | KademliaEvent::PendingRoutablePeer { .. }
                        | KademliaEvent::RoutingUpdated { .. } => {
                            // We are not interested in this event at the moment.
                        }
                        KademliaEvent::OutboundQueryProgressed {
                            result,
                            ..
                        } => {
                            match result {
                                QueryResult::Bootstrap(bootstrap_ok) => {
                                    match bootstrap_ok {
                                        Ok(ok) => {
                                            self.peer_to_remove
                                                .remove(&ok.peer);
                                        }
                                        Err(e) => {
                                            let BootstrapError::Timeout {
                                                peer,
                                                ..
                                            } = e;
                                            let count = self
                                                .peer_to_remove
                                                .entry(peer)
                                                .and_modify(|x| *x += 1)
                                                .or_default();

                                            if *count == 3 {
                                                self.remove_node(&peer);
                                                self.peer_to_remove
                                                    .remove(&peer);
                                            }
                                        }
                                    };
                                }
                                QueryResult::GetClosestPeers(
                                    get_closest_peers_ok,
                                ) => {
                                    // kademlia.get_closest_peers(PeerId)
                                    match get_closest_peers_ok {
                                        Ok(GetClosestPeersOk {
                                            key,
                                            peers,
                                        }) => {
                                            if let Ok(peer_id) =
                                                PeerId::from_bytes(&key)
                                            {
                                                if let Some(info) =
                                                    peers.iter().find(|x| {
                                                        x.peer_id == peer_id
                                                    })
                                                {
                                                    return Poll::Ready(
                                                    ToSwarm::GenerateEvent(
                                                        Event::ClosestPeer {
                                                            peer_id,
                                                            info: Some(
                                                                info.clone(),
                                                            ),
                                                        },
                                                    ),
                                                );
                                                } else {
                                                    return Poll::Ready(
                                                    ToSwarm::GenerateEvent(
                                                        Event::ClosestPeer {
                                                            peer_id,
                                                            info: None,
                                                        },
                                                    ),
                                                );
                                                }
                                            };
                                        }
                                        Err(
                                            GetClosestPeersError::Timeout {
                                                key,
                                                ..
                                            },
                                        ) => {
                                            if let Ok(peer_id) =
                                                PeerId::from_bytes(&key)
                                            {
                                                return Poll::Ready(
                                                    ToSwarm::GenerateEvent(
                                                        Event::ClosestPeer {
                                                            peer_id,
                                                            info: None,
                                                        },
                                                    ),
                                                );
                                            };
                                        }
                                    }
                                }
                                QueryResult::GetProviders(..)
                                | QueryResult::StartProviding(..)
                                | QueryResult::RepublishProvider(..)
                                | QueryResult::GetRecord(..)
                                | QueryResult::PutRecord(..)
                                | QueryResult::RepublishRecord(..) => {
                                    // We are not interested in this event at the moment.
                                }
                            };
                        }
                    }
                }
                ToSwarm::Dial { opts } => {
                    return Poll::Ready(ToSwarm::Dial { opts });
                }
                ToSwarm::NotifyHandler {
                    peer_id,
                    handler,
                    event,
                } => {
                    return Poll::Ready(ToSwarm::NotifyHandler {
                        peer_id,
                        handler,
                        event,
                    });
                }
                ToSwarm::CloseConnection {
                    peer_id,
                    connection,
                } => {
                    return Poll::Ready(ToSwarm::CloseConnection {
                        peer_id,
                        connection,
                    });
                }
                ToSwarm::ExternalAddrConfirmed(e) => {
                    return Poll::Ready(ToSwarm::ExternalAddrConfirmed(e));
                }
                ToSwarm::ExternalAddrExpired(e) => {
                    return Poll::Ready(ToSwarm::ExternalAddrExpired(e));
                }
                ToSwarm::ListenOn { opts } => {
                    return Poll::Ready(ToSwarm::ListenOn { opts });
                }
                ToSwarm::NewExternalAddrCandidate(e) => {
                    return Poll::Ready(ToSwarm::NewExternalAddrCandidate(e));
                }
                ToSwarm::RemoveListener { id } => {
                    return Poll::Ready(ToSwarm::RemoveListener { id });
                }
                _ => {}
            }
        }

        // Poll the stream that fires when we need to start a random Kademlia query.
        if !self.pre_routing {
            if let Some(next_random_walk) = self.next_random_walk.as_mut() {
                while next_random_walk.poll_unpin(cx).is_ready() {
                    if self.num_connections < self.discovery_only_if_under_num {
                        self.kademlia.get_closest_peers(PeerId::random());
                    }

                    // Schedule the next random query with exponentially increasing delay,
                    // capped at 60 seconds.
                    *next_random_walk = Delay::new(self.duration_to_next_kad);

                    self.duration_to_next_kad = cmp::min(
                        self.duration_to_next_kad * 2,
                        Duration::from_secs(60 * 2),
                    );
                }
            }
        }

        self.waker = Some(cx.waker().clone());
        Poll::Pending
    }

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), libp2p::swarm::ConnectionDenied> {
        self.kademlia.handle_pending_inbound_connection(
            connection_id,
            local_addr,
            remote_addr,
        )
    }

    fn handle_pending_outbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        maybe_peer: Option<PeerId>,
        addresses: &[Multiaddr],
        effective_role: libp2p::core::Endpoint,
    ) -> Result<Vec<Multiaddr>, libp2p::swarm::ConnectionDenied> {
        self.kademlia.handle_pending_outbound_connection(
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )
    }
}

/// Configuration for the routing behaviour.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Whether to enable random walks in the Kademlia DHT.
    dht_random_walk: bool,

    /// Number of active connections over which we interrupt the discovery process.
    discovery_only_if_under_num: u64,

    /// Whether to allow non-global addresses in the DHT.
    allow_non_globals_address_in_dht: bool,

    /// When enabled the number of disjoint paths used equals the configured parallelism.
    kademlia_disjoint_query_paths: bool,
}

impl Config {
    /// Creates a new configuration for the discovery behaviour.
    pub fn new() -> Self {
        Self {
            dht_random_walk: false,
            discovery_only_if_under_num: u64::MAX,
            allow_non_globals_address_in_dht: true,
            kademlia_disjoint_query_paths: true,
        }
    }

    /// Get DHT random walk.
    pub fn get_dht_random_walk(&self) -> bool {
        self.dht_random_walk
    }

    /// Enables or disables random walks in the Kademlia DHT.
    pub fn with_dht_random_walk(mut self, enable: bool) -> Self {
        self.dht_random_walk = enable;
        self
    }

    /// Get discovery limits.
    pub fn get_discovery_limit(&self) -> u64 {
        self.discovery_only_if_under_num
    }

    /// Sets the number of active connections over which we interrupt the discovery process.
    pub fn with_discovery_limit(mut self, num: u64) -> Self {
        self.discovery_only_if_under_num = num;
        self
    }

    /// Get allow_non_globals_in_dht.
    pub fn get_allow_non_globals_address_in_dht(&self) -> bool {
        self.allow_non_globals_address_in_dht
    }

    /// Whether to allow non-global addresses in the DHT.
    pub fn with_allow_non_globals_address_in_dht(
        mut self,
        allow: bool,
    ) -> Self {
        self.allow_non_globals_address_in_dht = allow;
        self
    }

    /// Get allow kademlia disjoint query paths
    pub fn get_kademlia_disjoint_query_paths(&self) -> bool {
        self.kademlia_disjoint_query_paths
    }

    /// When enabled the number of disjoint paths used equals the configured parallelism.
    pub fn with_kademlia_disjoint_query_paths(mut self, enable: bool) -> Self {
        self.kademlia_disjoint_query_paths = enable;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// A node in the routing table.
#[derive(Clone, Debug, Deserialize)]
pub struct RoutingNode {
    /// Peer ID.
    pub peer_id: String,
    /// Address.
    pub address: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::utils::convert_boot_nodes;

    use super::*;
    use test_log::test;

    use libp2p::{
        Multiaddr,
        swarm::{Swarm, SwarmEvent},
    };
    use libp2p_swarm_test::SwarmExt;

    use futures::prelude::*;
    use serial_test::serial;

    #[test(tokio::test)]
    #[serial]
    async fn test_routing() {
        let mut boot_nodes = vec![];

        let config = Config::new()
            .with_allow_non_globals_address_in_dht(true)
            .with_discovery_limit(100)
            .with_dht_random_walk(true);

        let (mut boot_swarm, addr) = build_node(config, 2000, HashMap::new());

        let peer_id = *boot_swarm.local_peer_id();
        let boot_node = RoutingNode {
            peer_id: peer_id.to_base58(),
            address: vec![addr.to_string()],
        };

        boot_nodes.push(boot_node);
        let boot_nodes: HashMap<PeerId, Vec<Multiaddr>> =
            convert_boot_nodes(&boot_nodes);

        let mut swarms = (1..10)
            .map(|x| {
                let config = Config::new()
                    .with_allow_non_globals_address_in_dht(true)
                    .with_discovery_limit(100);
                let (swarm, addr) =
                    build_node(config, 2000 + x, boot_nodes.clone());
                boot_swarm.behaviour_mut().add_self_reported_address(
                    swarm.local_peer_id(),
                    addr.clone(),
                );

                (swarm, addr)
            })
            .collect::<Vec<_>>();

        swarms.insert(0, (boot_swarm, addr));

        // Build a `Vec<HashSet<PeerId>>` with the list of nodes remaining to be discovered.
        let mut to_discover = (0..swarms.len())
            .map(|n| {
                (0..swarms.len())
                    // Skip the first swarm as all other swarms already know it.
                    .skip(1)
                    .filter(|p| *p != n)
                    .map(|p| *Swarm::local_peer_id(&swarms[p].0))
                    .collect::<HashSet<_>>()
            })
            .collect::<Vec<_>>();

        let fut = futures::future::poll_fn(move |cx| {
            loop {
                for swarm_n in 0..swarms.len() {
                    for peer_id in to_discover[swarm_n].clone() {
                        swarms[swarm_n].0.behaviour_mut().discover(&peer_id);
                    }

                    match swarms[swarm_n].0.poll_next_unpin(cx) {
                        Poll::Ready(Some(e)) => match e {
                            SwarmEvent::ConnectionEstablished {
                                peer_id,
                                ..
                            } => {
                                to_discover[swarm_n].remove(&peer_id);
                            }
                            SwarmEvent::Behaviour(behavior) => match behavior {
                                Event::ClosestPeer { peer_id, info } => {
                                    if let Some(info) = info {
                                        for addr in info.addrs {
                                            swarms[swarm_n]
                                                .0
                                                .behaviour_mut()
                                                .add_self_reported_address(
                                                    &peer_id, addr,
                                                );
                                        }

                                        if swarms[swarm_n]
                                            .0
                                            .behaviour_mut()
                                            .is_known_peer(&peer_id)
                                        {
                                            to_discover[swarm_n]
                                                .remove(&peer_id);
                                        }
                                    }
                                }
                            },
                            _ => {}
                        },
                        _ => {}
                    }
                }
                break;
            }
            if to_discover.iter().all(|l| l.is_empty()) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        });

        futures::executor::block_on(fut);
    }

    /// Build test swarm
    fn build_node(
        config: Config,
        port: u64,
        boot_nodes: HashMap<PeerId, Vec<Multiaddr>>,
    ) -> (Swarm<Behaviour>, Multiaddr) {
        let mut swarm = Swarm::new_ephemeral(|key_pair| {
            Behaviour::new(
                PeerId::from_public_key(&key_pair.public()),
                config,
                StreamProtocol::new("/kore/routing/1.0.0"),
                NodeType::Bootstrap,
            )
        });
        let listen_addr: Multiaddr =
            format!("/memory/{}", port).parse().unwrap();

        let _ = swarm.listen_on(listen_addr.clone()).unwrap();

        swarm.add_external_address(listen_addr.clone());

        for (peer_id, addrs) in boot_nodes {
            for addr in addrs {
                swarm
                    .behaviour_mut()
                    .add_self_reported_address(&peer_id, addr);
            }
        }

        swarm.behaviour_mut().finish_prerouting_state();

        (swarm, listen_addr)
    }
}
