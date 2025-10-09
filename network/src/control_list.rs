// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use libp2p::{
    Multiaddr, PeerId,
    swarm::{
        CloseConnection, ConnectionDenied, NetworkBehaviour, ToSwarm, dummy,
    },
};
use serde::Deserialize;
use std::{
    collections::{HashSet, VecDeque},
    fmt,
    pin::Pin,
    str::FromStr,
    task::Poll,
    time::Duration,
};
use tokio::{
    sync::mpsc::{self, Receiver},
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::{RoutingNode, utils::request_update_lists};

const TARGET_CONTROL_LIST: &str = "KoreNetwork-Control-list";

/// Configuration for the control list behaviour.
#[derive(Clone, Debug, Deserialize, Default)]
pub struct Config {
    /// Activate allow and block lists
    enable: bool,

    /// Nodes allowed to make and receive connections
    allow_list: Vec<String>,

    /// Nodes that are not allowed to make and receive connections
    block_list: Vec<String>,

    /// Services where the node will go to query the list of allowed nodes.
    service_allow_list: Vec<String>,

    /// Servicse where the node will go to query the list of blocked nodes.
    service_block_list: Vec<String>,

    /// Time interval to be used for queries updating the lists
    interval_request: Duration,
}

/// Control List Settings
impl Config {
    /// Set enable
    pub fn with_enable(mut self, enable: bool) -> Self {
        self.enable = enable;
        self
    }

    /// Set allow list
    pub fn with_allow_list(mut self, allow_list: Vec<String>) -> Self {
        self.allow_list = allow_list;
        self
    }

    /// Set block list
    pub fn with_block_list(mut self, block_list: Vec<String>) -> Self {
        self.block_list = block_list;
        self
    }

    /// Set Service list to consult allow list
    pub fn with_service_allow_list(
        mut self,
        service_allow_list: Vec<String>,
    ) -> Self {
        self.service_allow_list = service_allow_list;
        self
    }

    /// Set Service list to consult block list
    pub fn with_service_block_list(
        mut self,
        service_block_list: Vec<String>,
    ) -> Self {
        self.service_block_list = service_block_list;
        self
    }

    /// Set interval request
    pub fn with_interval_request(mut self, interval: Duration) -> Self {
        self.interval_request = interval;
        self
    }

    /// Set interval request
    pub fn get_interval_request(&self) -> Duration {
        self.interval_request
    }

    /// Get enable
    pub fn get_enable(&self) -> bool {
        self.enable
    }

    /// Get allow list
    pub fn get_allow_list(&self) -> Vec<String> {
        self.allow_list.clone()
    }

    /// Get block list
    pub fn get_block_list(&self) -> Vec<String> {
        self.block_list.clone()
    }

    /// Get Service list to consult allow list
    pub fn get_service_allow_list(&self) -> Vec<String> {
        self.service_allow_list.clone()
    }
    /// Get Service list to consult block list
    pub fn get_service_block_list(&self) -> Vec<String> {
        self.service_block_list.clone()
    }
}



async fn update_and_send_lists(
    service_allow: &[String],
    service_block: &[String],
    sender: &mpsc::Sender<Event>,
) {
    let (
        (vec_allow_peers, vec_block_peers),
        (successful_allow, successful_block),
    ) = request_update_lists(service_allow, service_block).await;

    // If at least 1 update of the list was possible
    if successful_allow != 0 {
        if let Err(e) = sender.send(Event::AllowListUpdated(vec_allow_peers)).await {
            error!(TARGET_CONTROL_LIST, "Can not send Event::AllowListUpdated, {}", e)
        }
    } else {
        warn!(
            TARGET_CONTROL_LIST,
            "No get to the services providing the list of allowed peers was performed."
        );
    }

    // If at least 1 update of the list was possible
    if successful_block != 0 {
        if let Err(e) = sender.send(Event::BlockListUpdated(vec_block_peers)).await {
            error!(TARGET_CONTROL_LIST, "Can not send Event::BlockListUpdated, {}", e)
        }
    } else {
        warn!(
            TARGET_CONTROL_LIST,
            "No get to the services providing the list of block peers was performed."
        );
    }
}

pub fn build_control_lists_updaters(
    config: &Config,
    token: CancellationToken,
) -> Option<Receiver<Event>> {
    if config.enable {
        info!(TARGET_CONTROL_LIST, "Control list is enable.");

        let (sender, receiver) = mpsc::channel(1000);
        let interval = config.interval_request;
        let service_allow = config.service_allow_list.clone();
        let service_block = config.service_block_list.clone();

        tokio::spawn(async move {
            // Execute initial request_update_lists
            update_and_send_lists(&service_allow, &service_block, &sender).await;

            // Then repeat at intervals
            loop {
                tokio::select! {
                    _ = sleep(interval) => {
                        update_and_send_lists(&service_allow, &service_block, &sender).await;
                    }
                    _ = token.cancelled() => {
                        info!(TARGET_CONTROL_LIST, "Control list updater cancelled.");
                        break;
                    }
                };
            }
        });

        Some(receiver)
    } else {
        info!(TARGET_CONTROL_LIST, "Control list is not enable.");
        None
    }
}

#[derive(Default, Debug)]
pub struct Behaviour {
    allow_peers: HashSet<PeerId>,
    block_peers: HashSet<PeerId>,
    close_connections: VecDeque<PeerId>,
    enable: bool,
    receiver: Option<Receiver<Event>>,
}

impl Behaviour {
    /// Creates a new control list `Behaviour`.
    pub fn new(
        config: Config,
        boot_nodes: &[RoutingNode],
        receiver: Option<Receiver<Event>>,
    ) -> Self {
        if config.enable {
            let mut full_allow_list = config.allow_list.clone();
            for node in boot_nodes {
                full_allow_list.push(node.peer_id.clone());
            }

            Self {
                enable: true,
                allow_peers: HashSet::from_iter(
                    full_allow_list
                        .iter()
                        .filter_map(|e| PeerId::from_str(e).ok()),
                ),
                block_peers: HashSet::from_iter(
                    config
                        .block_list
                        .iter()
                        .filter_map(|e| PeerId::from_str(e).ok()),
                ),
                receiver,
                ..Default::default()
            }
        } else {
            Behaviour::default()
        }
    }

    /// Method that update allow list
    fn update_allow_peers(&mut self, new_list: &[String]) {
        // New hashset of allow list.
        let new_list: HashSet<PeerId> = HashSet::from_iter(
            new_list
                .to_vec()
                .iter()
                .filter_map(|e| PeerId::from_str(e).ok()),
        );

        let close_peers: Vec<PeerId> =
            self.allow_peers.difference(&new_list).cloned().collect();
        self.close_connections.extend(close_peers.clone());
        self.allow_peers.clone_from(&new_list);
    }

    /// Method that update block list
    fn update_block_peers(&mut self, new_list: &[String]) {
        // New hashset of block list.
        let new_list: HashSet<PeerId> = HashSet::from_iter(
            new_list
                .to_vec()
                .iter()
                .filter_map(|e| PeerId::from_str(e).ok()),
        );

        self.close_connections.extend(new_list.clone());
        self.block_peers.clone_from(&new_list);
    }

    /// Method that check if a peer is in allow list
    fn check_allow(&self, peer: &PeerId) -> Result<(), ConnectionDenied> {
        if self.allow_peers.contains(peer) {
            return Ok(());
        }

        warn!(
            TARGET_CONTROL_LIST,
            "Node {} has been blocked, it is not in the allowed list.", peer
        );
        Err(ConnectionDenied::new(NotAllowed { peer: *peer }))
    }

    /// Method that check if a peer is in block list
    fn check_block(&self, peer: &PeerId) -> Result<(), ConnectionDenied> {
        if !self.block_peers.contains(peer) {
            return Ok(());
        }

        warn!(
            TARGET_CONTROL_LIST,
            "Node {} has been blocked, it is in the blocked list.", peer
        );
        Err(ConnectionDenied::new(Blocked { peer: *peer }))
    }

    /// Method that check all List
    fn check_lists(&self, peer: &PeerId) -> Result<(), ConnectionDenied> {
        if self.enable {
            self.check_block(peer)?;
            self.check_allow(peer)?;
        }

        Ok(())
    }
}

/// A connection to this peer is not explicitly allowed and was thus [`denied`](ConnectionDenied).
#[derive(Debug)]
pub struct NotAllowed {
    peer: PeerId,
}

impl fmt::Display for NotAllowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "peer {} is not in the allow list", self.peer)
    }
}

impl std::error::Error for NotAllowed {}

/// A connection to this peer was explicitly blocked and was thus [`denied`](ConnectionDenied).
#[derive(Debug)]
pub struct Blocked {
    peer: PeerId,
}

impl fmt::Display for Blocked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "peer {} is in the block list", self.peer)
    }
}

impl std::error::Error for Blocked {}

/// Event Struct for implement control list Behaviour in main Behaviour
#[derive(Debug)]
pub enum Event {
    AllowListUpdated(Vec<String>),
    BlockListUpdated(Vec<String>),
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = Event;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _: &libp2p::Multiaddr,
        _: &libp2p::Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, ConnectionDenied> {
        self.check_lists(&peer)?;

        Ok(dummy::ConnectionHandler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _: libp2p::swarm::ConnectionId,
        peer: Option<PeerId>,
        _: &[libp2p::Multiaddr],
        _: libp2p::core::Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        if let Some(peer) = peer {
            self.check_lists(&peer)?;
        }

        Ok(vec![])
    }

    fn handle_established_outbound_connection(
        &mut self,
        _: libp2p::swarm::ConnectionId,
        peer: PeerId,
        _: &libp2p::Multiaddr,
        _: libp2p::core::Endpoint,
        _: libp2p::core::transport::PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, ConnectionDenied> {
        self.check_lists(&peer)?;

        Ok(dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, _: libp2p::swarm::FromSwarm) {}

    fn on_connection_handler_event(
        &mut self,
        _: PeerId,
        _: libp2p::swarm::ConnectionId,
        _: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
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
        let mut receiver_opt = self.receiver.take();
        if let Some(mut rx) = receiver_opt.as_mut() {
            let mut cx = std::task::Context::from_waker(cx.waker());
            while let Poll::Ready(Some(event)) =
                Pin::new(&mut rx).poll_recv(&mut cx)
            {
                match event {
                    Event::AllowListUpdated(items) => {
                        self.update_allow_peers(&items)
                    }
                    Event::BlockListUpdated(items) => {
                        self.update_block_peers(&items)
                    }
                }
            }
        }

        self.receiver = receiver_opt;

        if let Some(peer) = self.close_connections.pop_front() {
            return Poll::Ready(ToSwarm::CloseConnection {
                peer_id: peer,
                connection: CloseConnection::All,
            });
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;
    use libp2p::{
        Swarm,
        swarm::{
            ConnectionError, DialError, ListenError, SwarmEvent,
            dial_opts::DialOpts,
        },
    };
    use libp2p_swarm_test::SwarmExt;
    use serial_test::serial;
    use test_log::test;

    use super::*;

    impl Behaviour {
        pub fn block_peer(&mut self, peer: PeerId) {
            self.block_peers.insert(peer);
            self.close_connections.push_back(peer);
        }

        pub fn allow_peer(&mut self, peer: PeerId) {
            self.allow_peers.insert(peer);
        }
        pub fn set_enable(&mut self, enable: bool) {
            self.enable = enable;
        }
    }

    fn dial(
        dialer: &mut Swarm<Behaviour>,
        listener: &Swarm<Behaviour>,
    ) -> Result<(), DialError> {
        dialer.dial(
            DialOpts::peer_id(*listener.local_peer_id())
                .addresses(listener.external_addresses().cloned().collect())
                .build(),
        )
    }

    fn build_behaviours() -> (Swarm<Behaviour>, Swarm<Behaviour>) {
        let mut behaviour = Behaviour::default();
        behaviour.set_enable(true);
        let dialer = Swarm::new_ephemeral(|_| behaviour);

        let mut behaviour = Behaviour::default();
        behaviour.set_enable(true);
        let listener = Swarm::new_ephemeral(|_| behaviour);

        (dialer, listener)
    }

    #[test(tokio::test)]
    #[serial]
    async fn cannot_dial_blocked_peer() {
        let (mut dialer, mut listener) = build_behaviours();

        listener.listen().with_memory_addr_external().await;

        dialer.behaviour_mut().block_peer(*listener.local_peer_id());

        let DialError::Denied { cause } =
            dial(&mut dialer, &listener).unwrap_err()
        else {
            panic!("unexpected dial error")
        };
        assert!(cause.downcast::<Blocked>().is_ok());
    }

    #[test(tokio::test)]
    #[serial]
    async fn cannot_dial_not_allowed_peer() {
        let (mut dialer, mut listener) = build_behaviours();

        listener.listen().with_memory_addr_external().await;

        let DialError::Denied { cause } =
            dial(&mut dialer, &listener).unwrap_err()
        else {
            panic!("unexpected dial error")
        };
        assert!(cause.downcast::<NotAllowed>().is_ok());
    }

    #[test(tokio::test)]
    #[serial]
    async fn can_dial_allowed_not_blocked_peer() {
        let (mut dialer, mut listener) = build_behaviours();

        listener.listen().with_memory_addr_external().await;

        dialer.behaviour_mut().allow_peer(*listener.local_peer_id());

        dial(&mut dialer, &listener).unwrap();
    }

    #[test(tokio::test)]
    #[serial]
    async fn cannot_dial_allowed_blocked_peer() {
        let (mut dialer, mut listener) = build_behaviours();
        listener.listen().with_memory_addr_external().await;

        dialer.behaviour_mut().block_peer(*listener.local_peer_id());
        dialer.behaviour_mut().allow_peer(*listener.local_peer_id());

        let DialError::Denied { cause } =
            dial(&mut dialer, &listener).unwrap_err()
        else {
            panic!("unexpected dial error")
        };
        assert!(cause.downcast::<Blocked>().is_ok());
    }

    #[test(tokio::test)]
    #[serial]
    async fn blocked_peer_cannot_dial_us() {
        let (mut dialer, mut listener) = build_behaviours();
        listener.listen().with_memory_addr_external().await;

        dialer.behaviour_mut().allow_peer(*listener.local_peer_id());
        listener.behaviour_mut().block_peer(*dialer.local_peer_id());

        dial(&mut dialer, &listener).unwrap();
        tokio::spawn(dialer.loop_on_next());

        let cause = listener
            .wait(|e| match e {
                SwarmEvent::IncomingConnectionError {
                    error: ListenError::Denied { cause },
                    ..
                } => Some(cause),
                _ => None,
            })
            .await;
        assert!(cause.downcast::<Blocked>().is_ok());
    }

    #[test(tokio::test)]
    #[serial]
    async fn not_allowed_peer_cannot_dial_us() {
        let (mut dialer, mut listener) = build_behaviours();
        listener.listen().with_memory_addr_external().await;

        dialer.behaviour_mut().allow_peer(*listener.local_peer_id());

        dial(&mut dialer, &listener).unwrap();

        let listener_loop = async move {
            loop {
                match listener.select_next_some().await {
                    SwarmEvent::IncomingConnectionError { error, .. } => {
                        let ListenError::Denied { cause } = error else {
                            panic!("Invalid Error")
                        };
                        assert!(cause.downcast::<NotAllowed>().is_ok());
                        break;
                    }
                    _ => {}
                }
            }
        };

        let dialer_loop = async move {
            loop {
                match dialer.select_next_some().await {
                    SwarmEvent::ConnectionClosed { cause, .. } => {
                        if let Some(error) = cause {
                            match error {
                                ConnectionError::IO(e) => {
                                    assert_eq!(
                                        e.to_string(),
                                        "Right(Io(Kind(BrokenPipe)))"
                                    );
                                    break;
                                }
                                _ => {
                                    panic!("Invalid error");
                                }
                            }
                        } else {
                            panic!("Missing error");
                        };
                    }
                    _ => {}
                }
            }
        };
        tokio::task::spawn(Box::pin(dialer_loop));
        listener_loop.await;
    }

    #[test(tokio::test)]
    #[serial]
    async fn connections_get_closed_upon_disallow() {
        let (mut dialer, mut listener) = build_behaviours();
        listener.listen().with_memory_addr_external().await;

        dialer.behaviour_mut().allow_peer(*listener.local_peer_id());
        listener.behaviour_mut().allow_peer(*dialer.local_peer_id());
        let dialer_peer = *dialer.local_peer_id();

        dial(&mut dialer, &listener).unwrap();

        let listener_loop = async move {
            loop {
                match listener.select_next_some().await {
                    SwarmEvent::ConnectionEstablished { .. } => {
                        listener.behaviour_mut().block_peer(dialer_peer);
                    }
                    SwarmEvent::ConnectionClosed { .. } => {
                        break;
                    }
                    _ => {}
                }
            }
        };

        let dialer_loop = async move {
            loop {
                match dialer.select_next_some().await {
                    SwarmEvent::ConnectionEstablished { .. } => {}
                    SwarmEvent::ConnectionClosed { cause, .. } => {
                        if let Some(error) = cause {
                            match error {
                                ConnectionError::IO(e) => {
                                    assert_eq!(e.to_string(), "Right(Closed)");
                                    break;
                                }
                                _ => {
                                    panic!("Invalid error");
                                }
                            }
                        } else {
                            panic!("Missing error");
                        };
                    }
                    _ => {}
                }
            }
        };

        tokio::task::spawn(Box::pin(dialer_loop));
        listener_loop.await;
    }
}
