// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Network worker.
//!

use crate::{
    Command, CommandHelper, Config, Error, Event as NetworkEvent, Monitor,
    MonitorMessage, NodeType,
    behaviour::{Behaviour, Event as BehaviourEvent, ReqResMessage},
    service::NetworkService,
    transport::build_transport,
    utils::{
        Fact, MessagesHelper, MetricLabels, NetworkState, convert_addresses,
        convert_boot_nodes,
    },
};

use std::{collections::HashSet, fmt::Debug, time::Duration};

use actor::ActorRef;
use identity::{
    identifier::derive::KeyDerivator,
    keys::{KeyMaterial, KeyPair},
};

use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm,
    core::ConnectedPoint,
    identity::{Keypair, ed25519, secp256k1},
    request_response::{self, ResponseChannel},
    swarm::{
        self, ConnectionError, DialError, SwarmEvent, dial_opts::DialOpts,
    },
};

use futures::StreamExt;
use prometheus_client::{
    metrics::{counter::Counter, family::Family},
    registry::Registry,
};
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, trace, warn};

use std::collections::{HashMap, VecDeque};

const TARGET_WORKER: &str = "KoreNetwork-Worker";

/// Main network worker. Must be polled in order for the network to advance.
///
/// The worker is responsible for handling the network events and commands.
///
pub struct NetworkWorker<T>
where
    T: Debug + Serialize,
{
    /// Local Peer ID.
    local_peer_id: PeerId,

    /// Network service.
    service: NetworkService,

    /// The libp2p swarm.
    swarm: Swarm<Behaviour>,

    /// The network state.
    state: NetworkState,

    /// The command receiver.
    command_receiver: mpsc::Receiver<Command>,

    /// The command sender to Helper Intermediary.
    helper_sender: Option<mpsc::Sender<CommandHelper<T>>>,

    /// Monitor actor.
    monitor: Option<ActorRef<Monitor>>,

    /// The cancellation token.
    cancel: CancellationToken,

    /// Node type.
    node_type: NodeType,

    /// List of boot noodes.
    boot_nodes: HashMap<PeerId, Vec<Multiaddr>>,

    /// nodes with which it has not been possible to establish a connection by keepAliveTimeout in pre-routing.
    retry_boot_nodes: HashMap<PeerId, Vec<Multiaddr>>,

    retrys: u8,

    /// Pendings outbound messages to the peer
    pending_outbound_messages: HashMap<PeerId, VecDeque<Vec<u8>>>,

    pending_inbound_messages: HashMap<PeerId, VecDeque<Vec<u8>>>,

    /// Ephemeral responses.
    ephemeral_responses:
        HashMap<PeerId, VecDeque<ResponseChannel<ReqResMessage>>>,

    /// Messages metric.
    messages_metric: Family<MetricLabels, Counter>,

    /// Successful dials
    successful_dials: u64,

    protocols: HashSet<StreamProtocol>,

    try_to_send_message: HashMap<PeerId, u8>,

    pending_dial: HashMap<PeerId, Vec<Multiaddr>>,

    peer_identify: HashSet<PeerId>,
}

impl<T: Debug + Serialize> NetworkWorker<T> {
    /// Create a new `NetworkWorker`.
    pub fn new(
        registry: &mut Registry,
        keys: KeyPair,
        config: Config,
        monitor: Option<ActorRef<Monitor>>,
        keyderivator: KeyDerivator,
        cancel: CancellationToken,
    ) -> Result<Self, Error> {
        // Create channels to communicate commands
        info!(TARGET_WORKER, "Creating network");
        let (command_sender, command_receiver) = mpsc::channel(100000);

        // Prepare the network crypto key.
        let key = match keyderivator {
            KeyDerivator::Ed25519 => {
                let sk =
                    ed25519::SecretKey::try_from_bytes(keys.secret_key_bytes())
                        .map_err(|e| {
                            Error::Worker(format!(
                                "Invalid Ed25518 secret key {}",
                                e
                            ))
                        })?;
                let kp = ed25519::Keypair::from(sk);
                Keypair::from(kp)
            }
            KeyDerivator::Secp256k1 => {
                let sk = secp256k1::SecretKey::try_from_bytes(
                    keys.secret_key_bytes(),
                )
                .map_err(|e| {
                    Error::Worker(format!("Invalid Secp256k1 secret key {}", e))
                })?;
                let kp = secp256k1::Keypair::from(sk);
                Keypair::from(kp)
            }
        };

        // Generate the `PeerId` from the public key.
        let local_peer_id = key.public().to_peer_id();

        let boot_nodes = convert_boot_nodes(&config.boot_nodes);

        // Create the listen addressess.
        let addresses = convert_addresses(&config.listen_addresses)?;

        // Create the listen addressess.
        let external_addresses = convert_addresses(&config.external_addresses)?;

        // Is Ephemeral?
        let node_type = config.node_type.clone();

        // Build transport.
        let transport = build_transport(registry, &key)?;

        let (behaviour, protocols) =
            Behaviour::build(&key.public(), config.clone(), cancel.clone());

        // Create the swarm.
        let mut swarm = Swarm::new(
            transport,
            behaviour,
            local_peer_id,
            swarm::Config::with_tokio_executor()
                .with_idle_connection_timeout(Duration::from_secs(5)),
        );

        // Register metrics
        let messages_metric = Family::default();
        registry.register(
            "Messages",
            "Counts messages sent or received from other peers.",
            messages_metric.clone(),
        );

        let service = NetworkService::new(command_sender);

        if addresses.is_empty() {
            // Listen on all tcp addresses.
            swarm
                .listen_on(
                    "/ip4/0.0.0.0/tcp/0"
                        .parse::<Multiaddr>()
                        .map_err(|e| Error::Address(e.to_string()))?,
                )
                .map_err(|e| {
                    Error::Address(format!(
                        "Error listening on all interfaces: {}",
                        e
                    ))
                })?;
            info!(TARGET_WORKER, "Listen in all interfaces");
        } else {
            // Listen on the external addresses.
            for addr in addresses.iter() {
                info!(TARGET_WORKER, "Listen in {}", addr);
                swarm.listen_on(addr.clone()).map_err(|e| Error::Worker(format!("Transport does not support the listening addresss: {}: {}", addr, e)))?;
            }
        }

        if !external_addresses.is_empty() {
            for addr in external_addresses.iter() {
                info!(TARGET_WORKER, "Add external address {:?}", addr);
                swarm.add_external_address(addr.clone());
            }
        }

        info!(TARGET_WORKER, "LOCAL PEER-ID {}", local_peer_id);
        Ok(Self {
            local_peer_id,
            service,
            swarm,
            state: NetworkState::Start,
            retrys: 0,
            command_receiver,
            helper_sender: None,
            monitor,
            cancel,
            node_type,
            boot_nodes,
            retry_boot_nodes: HashMap::new(),
            pending_outbound_messages: HashMap::default(),
            pending_inbound_messages: HashMap::default(),
            ephemeral_responses: HashMap::default(),
            messages_metric,
            successful_dials: 0,
            protocols,
            try_to_send_message: HashMap::new(),
            pending_dial: HashMap::new(),
            peer_identify: HashSet::new(),
        })
    }

    /// Add sender helper
    pub fn add_helper_sender(
        &mut self,
        helper_sender: mpsc::Sender<CommandHelper<T>>,
    ) {
        self.helper_sender = Some(helper_sender);
    }

    /// Get the local peer ID.
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Send message to a peer.
    ///
    ///
    fn send_message(
        &mut self,
        peer: PeerId,
        message: Vec<u8>,
    ) -> Result<(), Error> {
        if let Some(responses) = self.ephemeral_responses.get_mut(&peer) {
            while let Some(response_channel) = responses.pop_front() {
                if let Err(e) = self
                    .swarm
                    .behaviour_mut()
                    .send_response(response_channel, message.clone())
                {
                    warn!(
                        TARGET_WORKER,
                        "Can not send response to {}: {}", peer, e
                    )
                } else {
                    self.messages_metric
                        .get_or_create(&MetricLabels {
                            fact: Fact::Sent,
                            peer_id: peer.to_string(),
                        })
                        .inc();

                    break;
                }
            }

            if responses.is_empty() {
                self.ephemeral_responses.remove(&peer);
            }
        } else {
            self.add_pending_outbound_message(peer, message.clone());

            if self.swarm.behaviour_mut().is_known_peer(&peer) {
                self.send_pending_outbound_messages(peer);
            } else {
                self.swarm.behaviour_mut().discover(&peer);
            }
        }

        Ok(())
    }

    /// Add pending message to peer.
    fn add_pending_outbound_message(&mut self, peer: PeerId, message: Vec<u8>) {
        let pending_messages: &mut VecDeque<Vec<u8>> =
            self.pending_outbound_messages.entry(peer).or_default();
        pending_messages.push_back(message);
    }

    /// Add ephemeral response.
    fn add_ephemeral_response(
        &mut self,
        peer: PeerId,
        response_channel: ResponseChannel<ReqResMessage>,
    ) {
        self.ephemeral_responses
            .entry(peer)
            .or_default()
            .push_back(response_channel);
    }

    /// Send pending messages to peer.
    fn send_pending_outbound_messages(&mut self, peer: PeerId) {
        if let Some(messages) = self.pending_outbound_messages.remove(&peer) {
            for message in messages.iter() {
                self.swarm
                    .behaviour_mut()
                    .send_message(&peer, message.clone());
            }
        }

        self.try_to_send_message.remove(&peer);
        self.pending_dial.remove(&peer);

        self.messages_metric
            .get_or_create(&MetricLabels {
                fact: Fact::Sent,
                peer_id: peer.to_string(),
            })
            .inc();
    }

    /// Get the network service.
    pub fn service(&self) -> NetworkService {
        self.service.clone()
    }

    /// Change the network state.
    async fn change_state(&mut self, state: NetworkState) {
        trace!(TARGET_WORKER, "Change network state to: {:?}", state);
        self.state = state.clone();
        self.send_event(NetworkEvent::StateChanged(state)).await;
    }

    /// Send event
    async fn send_event(&mut self, event: NetworkEvent) {
        if let Some(monitor) = self.monitor.clone() {
            if let Err(e) = monitor.tell(MonitorMessage::Network(event)).await {
                error!(
                    TARGET_WORKER,
                    "Can't send network event to monitor actor: {}", e
                );
                self.cancel.cancel();
            }
        }
    }

    /// Run the network worker.
    pub async fn run(&mut self) {
        // Run connection to bootstrap node.
        if let Err(error) = self.run_connection().await {
            error!(TARGET_WORKER, "Error running connection: {:?}", error);
            self.send_event(NetworkEvent::Error(error)).await;
            // Irrecoverable error. Cancel the node.
            self.cancel.cancel();
            return;
        }

        self.send_event(NetworkEvent::StateChanged(NetworkState::Running))
            .await;

        // Finish pre routing state, activating random walk (if node is a bootstrap).
        self.swarm.behaviour_mut().finish_prerouting_state();
        // Run main loop.
        self.run_main().await;
    }

    /// Run connection to bootstrap node.
    pub async fn run_connection(&mut self) -> Result<(), Error> {
        info!(TARGET_WORKER, "Running connection loop");
        // If is the first node of kore network.
        if self.node_type == NodeType::Bootstrap && self.boot_nodes.is_empty() {
            self.change_state(NetworkState::Running).await;
            Ok(())
        } else {
            self.change_state(NetworkState::Dial).await;

            loop {
                match self.state {
                    NetworkState::Dial => {
                        // Dial to boot node.
                        if self.boot_nodes.is_empty() {
                            error!(TARGET_WORKER, "No bootstrap nodes.");
                            self.send_event(NetworkEvent::Error(
                                Error::Network(
                                    "No more bootstrap nodes.".to_owned(),
                                ),
                            ))
                            .await;

                            error!(
                                TARGET_WORKER,
                                "Can't connect to kore network"
                            );
                            self.change_state(NetworkState::Disconnected).await;
                        } else {
                            let copy_boot_nodes = self.boot_nodes.clone();

                            for (peer, addresses) in copy_boot_nodes {
                                if let Err(e) = self.swarm.dial(
                                    DialOpts::peer_id(peer)
                                        .addresses(addresses.clone())
                                        .build(),
                                ) {
                                    let (add_to_retry, new_addresses) =
                                        Self::dial_error_manager(
                                            e,
                                            peer,
                                            self.retrys,
                                        );

                                    self.boot_nodes.remove(&peer);
                                    if add_to_retry {
                                        if new_addresses.is_empty() {
                                            self.retry_boot_nodes
                                                .insert(peer, addresses);
                                        } else {
                                            self.retry_boot_nodes
                                                .insert(peer, new_addresses);
                                        }
                                    }
                                }
                            }
                            if !self.boot_nodes.is_empty() {
                                self.change_state(NetworkState::Dialing).await;
                            } else {
                                error!(TARGET_WORKER, "All dials fails");
                                self.change_state(NetworkState::Disconnected)
                                    .await;
                            }
                        }
                    }
                    NetworkState::Dialing => {
                        // No more bootnodes to send dial, none was successful nut one or more Dial fail by keepalivetimeout
                        if self.boot_nodes.is_empty()
                            && self.successful_dials == 0
                            && !self.retry_boot_nodes.is_empty()
                            && self.retrys < 3
                        {
                            info!(
                                TARGET_WORKER,
                                "Making a new retry: {}", self.retrys
                            );

                            self.retrys += 1;
                            self.boot_nodes.clone_from(&self.retry_boot_nodes);
                            self.retry_boot_nodes.clear();
                            self.change_state(NetworkState::Dial).await;
                        }
                        // No more bootnodes to send dial and none was successful
                        else if self.boot_nodes.is_empty()
                            && self.successful_dials == 0
                        {
                            self.change_state(NetworkState::Disconnected).await;
                        // No more bootnodes to send dial and one or more was successful
                        } else if self.boot_nodes.is_empty() {
                            return Ok(());
                        }
                    }
                    NetworkState::Running => {
                        return Ok(());
                    }
                    NetworkState::Disconnected => {
                        return Err(Error::Network(
                            "Can't connect to kore network".to_owned(),
                        ));
                    }
                    _ => {}
                }
                if self.state != NetworkState::Disconnected {
                    tokio::select! {
                        event = self.swarm.select_next_some() => {
                            self.handle_connection_events(event).await;
                        }
                        _ = self.cancel.cancelled() => {
                            return Err(Error::Network("Token cancellled".to_owned()));
                        }
                    }
                }
            }
        }
    }

    fn dial_error_manager(
        e: DialError,
        peer: PeerId,
        retrys: u8,
    ) -> (bool, Vec<Multiaddr>) {
        match e {
            DialError::LocalPeerId { .. } => {
                error!(
                    TARGET_WORKER,
                    "Error dialing, try: {}, peer-id: {}, The peer identity obtained on the connection matches the local peer.",
                    retrys,
                    peer
                );
            }
            DialError::NoAddresses => {
                error!(
                    TARGET_WORKER,
                    "Error dialing, try: {}, peer-id: {}, No addresses have been provided.",
                    retrys,
                    peer
                );
            }
            DialError::DialPeerConditionFalse(peer_condition) => {
                error!(
                    TARGET_WORKER,
                    "Error dialing, try: {}, peer-id: {}, The provided {:?} evaluated to false and thus the dial was aborted.",
                    retrys,
                    peer,
                    peer_condition
                );
            }
            DialError::Denied { cause } => {
                error!(
                    TARGET_WORKER,
                    "Error dialing, try: {}, peer-id: {}, One of the NetworkBehaviours rejected the outbound connection: {}",
                    retrys,
                    peer,
                    cause
                );
            }
            DialError::Aborted => {
                if retrys == 0 {
                    warn!(
                        TARGET_WORKER,
                        "Error dialing, try: {}, peer-id: {}, Pending connection attempt has been aborted, retry one more time",
                        retrys,
                        peer
                    );

                    return (true, vec![]);
                } else {
                    error!(
                        TARGET_WORKER,
                        "Error dialing, try: {}, peer-id: {}, Pending connection attempt has been aborted",
                        retrys,
                        peer
                    );
                }
            }
            DialError::WrongPeerId { obtained, .. } => {
                error!(
                    TARGET_WORKER,
                    "Error dialing, try: {}, peer-id: {}, The peer identity obtained on the connection did not match the one that was expected: obtained peer-id -> {}",
                    retrys,
                    peer,
                    obtained
                );
            }
            DialError::Transport(items) => {
                error!(
                    TARGET_WORKER,
                    "Error dialing, try: {}, peer-id: {}, Transport error, evaluating the error for each address",
                    retrys,
                    peer
                );

                let mut new_addresses = vec![];

                for (address, error) in items {
                    warn!(
                        TARGET_WORKER,
                        "Error: {}, address: {}", error, address,
                    );
                    match error {
                        libp2p::TransportError::MultiaddrNotSupported(..) => {}
                        libp2p::TransportError::Other(e) => {
                            let kind_error = e.kind();
                            match kind_error {
                                std::io::ErrorKind::ConnectionRefused
                                | std::io::ErrorKind::TimedOut
                                | std::io::ErrorKind::ConnectionAborted
                                | std::io::ErrorKind::NotConnected
                                | std::io::ErrorKind::BrokenPipe
                                | std::io::ErrorKind::Interrupted
                                | std::io::ErrorKind::HostUnreachable
                                | std::io::ErrorKind::NetworkUnreachable => {
                                    new_addresses.push(address);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                if !new_addresses.is_empty() {
                    return (true, new_addresses);
                }
            }
        }

        (false, vec![])
    }

    /// Handle connection events.
    async fn handle_connection_events(
        &mut self,
        event: SwarmEvent<BehaviourEvent>,
    ) {
        match event {
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                self.boot_nodes.remove(&peer_id);
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(peer_id),
                error,
                ..
            } => {
                let (add_to_retry, new_addresses) =
                    Self::dial_error_manager(error, peer_id, self.retrys);

                if let Some(addresses) = self.boot_nodes.remove(&peer_id) {
                    if add_to_retry {
                        if new_addresses.is_empty() {
                            self.retry_boot_nodes.insert(peer_id, addresses);
                        } else {
                            self.retry_boot_nodes
                                .insert(peer_id, new_addresses);
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Identified {
                peer_id,
                info,
                connection_id,
            }) => {
                if !self
                    .check_protocols(&info.protocol_version, &info.protocols)
                {
                    warn!(
                        TARGET_WORKER,
                        "Invalid protocols, peer-id: {}, protocols: {:?}, protocol-version: {}",
                        peer_id,
                        info.protocols,
                        info.protocol_version
                    );

                    self.swarm
                        .behaviour_mut()
                        .close_connections(&peer_id, Some(connection_id));
                } else {
                    for addr in info.listen_addrs {
                        self.swarm
                            .behaviour_mut()
                            .add_self_reported_address(&peer_id, addr);
                    }

                    self.successful_dials += 1;
                    self.peer_identify.insert(peer_id);
                    self.boot_nodes.remove(&peer_id);
                }
            }
            _ => {}
        }
    }

    fn clear_pending_messages(&mut self, peer_id: &PeerId) {
        self.pending_outbound_messages.remove(peer_id);
        self.try_to_send_message.remove(peer_id);
        self.pending_dial.remove(peer_id);
    }

    fn check_protocols(
        &self,
        protocol_version: &str,
        protocols: &[StreamProtocol],
    ) -> bool {
        let supp_protocols: HashSet<StreamProtocol> = protocols
            .iter()
            .cloned()
            .collect::<HashSet<StreamProtocol>>();

        protocol_version == "/kore/1.0.0"
            && self.protocols.is_subset(&supp_protocols)
    }

    /// Run network worker.
    pub async fn run_main(&mut self) {
        info!(TARGET_WORKER, "Running main loop");

        loop {
            tokio::select! {
                command = self.command_receiver.recv() => {
                    // Handle commands.
                    if let Some(command) = command {
                        self.handle_command(command).await;
                    }
                }
                event = self.swarm.select_next_some() => {
                    // Handle events.
                    self.handle_event(event).await;
                }
                _ = self.cancel.cancelled() => {
                    break;
                }
            }
        }
    }

    async fn handle_command(&mut self, command: Command) {
        match command {
            Command::SendMessage { peer, message } => {
                if let Err(error) = self.send_message(peer, message) {
                    error!(TARGET_WORKER, "Response error: {:?}", error);
                    self.send_event(NetworkEvent::Error(error)).await;
                }
            }
        }
    }

    async fn message_to_helper(&mut self, message: MessagesHelper) {
        'Send: {
            if let Some(helper_sender) = self.helper_sender.as_ref() {
                match message {
                    MessagesHelper::Single(items) => {
                        if helper_sender
                            .send(CommandHelper::ReceivedMessage {
                                message: items,
                            })
                            .await
                            .is_err()
                        {
                            break 'Send;
                        }
                    }
                    MessagesHelper::Vec(items) => {
                        for item in items {
                            if helper_sender
                                .send(CommandHelper::ReceivedMessage {
                                    message: item,
                                })
                                .await
                                .is_err()
                            {
                                break 'Send;
                            }
                        }
                    }
                }

                return;
            }
        }

        error!(TARGET_WORKER, "Could not send message to network helper");
        self.cancel.cancel();
    }

    async fn handle_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::Behaviour(event) => {
                match event {
                    BehaviourEvent::Identified {
                        peer_id,
                        info,
                        connection_id,
                    } => {
                        if !self.check_protocols(
                            &info.protocol_version,
                            &info.protocols,
                        ) {
                            warn!(
                                TARGET_WORKER,
                                "Invalid protocols, peer-id: {}, protocols: {:?}, protocol-version: {}",
                                peer_id,
                                info.protocols,
                                info.protocol_version
                            );

                            self.clear_pending_messages(&peer_id);

                            self.swarm.behaviour_mut().close_connections(
                                &peer_id,
                                Some(connection_id),
                            );
                        } else {
                            for addr in info.listen_addrs {
                                self.swarm
                                    .behaviour_mut()
                                    .add_self_reported_address(&peer_id, addr);
                            }

                            self.peer_identify.insert(peer_id);

                            if let Some(messages) =
                                self.pending_inbound_messages.get(&peer_id)
                            {
                                self.message_to_helper(MessagesHelper::Vec(
                                    messages.clone(),
                                ))
                                .await;
                            };

                            self.send_pending_outbound_messages(peer_id);
                        }
                    }
                    BehaviourEvent::IdentifyError { peer_id, error } => {
                        error!(
                            TARGET_WORKER,
                            "IdentifyError with peer_id: {}, error: {}",
                            peer_id,
                            error
                        );

                        match error {
                            swarm::StreamUpgradeError::Timeout => {
                                // We do not clean since we will try to open the connection when it is
                                // confirmed that it has been closed in SwarmEvent::ConnectionClosed
                            }
                            swarm::StreamUpgradeError::Apply(..)
                            | swarm::StreamUpgradeError::NegotiationFailed
                            | swarm::StreamUpgradeError::Io(..) => {
                                self.clear_pending_messages(&peer_id);
                            }
                        }

                        self.swarm
                            .behaviour_mut()
                            .close_connections(&peer_id, None);
                    }
                    BehaviourEvent::ReqresMessage { peer_id, message } => {
                        let message_data = match message {
                            request_response::Message::Request {
                                request,
                                channel,
                                ..
                            } => {
                                self.add_ephemeral_response(peer_id, channel);
                                request.0
                            }
                            request_response::Message::Response {
                                response,
                                ..
                            } => response.0,
                        };

                        if self.peer_identify.contains(&peer_id) {
                            self.message_to_helper(MessagesHelper::Single(
                                message_data,
                            ))
                            .await;
                        } else {
                            self.pending_inbound_messages
                                .entry(peer_id)
                                .or_default()
                                .push_back(message_data);
                        }

                        self.messages_metric
                            .get_or_create(&MetricLabels {
                                fact: Fact::Received,
                                peer_id: peer_id.to_string(),
                            })
                            .inc();
                    }
                    BehaviourEvent::TellMessage { peer_id, message } => {
                        if self.peer_identify.contains(&peer_id) {
                            self.message_to_helper(MessagesHelper::Single(
                                message.message,
                            ))
                            .await;
                        } else {
                            self.pending_inbound_messages
                                .entry(peer_id)
                                .or_default()
                                .push_back(message.message);
                        }

                        self.messages_metric
                            .get_or_create(&MetricLabels {
                                fact: Fact::Received,
                                peer_id: peer_id.to_string(),
                            })
                            .inc();
                    }
                    BehaviourEvent::ClosestPeer { peer_id, info } => {
                        let count = *self
                            .try_to_send_message
                            .entry(peer_id)
                            .and_modify(|x| *x += 1)
                            .or_default();

                        let error = if let Some(info) = info {
                            if let Err(e) = self.swarm.dial(
                                DialOpts::peer_id(peer_id)
                                    .addresses(info.addrs.clone())
                                    .build(),
                            ) {
                                Self::dial_error_manager(e, peer_id, count);
                                true
                            } else {
                                self.pending_dial.insert(peer_id, info.addrs);
                                false
                            }
                        } else {
                            true
                        };

                        if error {
                            if count >= 3 {
                                self.clear_pending_messages(&peer_id);
                            } else {
                                self.swarm.behaviour_mut().discover(&peer_id);
                            }
                        }
                    }
                    BehaviourEvent::Dummy => {
                        // For contron_list, ReqRes, Tell events
                    }
                }
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => match endpoint {
                ConnectedPoint::Dialer { address, .. } => {
                    info!(
                        TARGET_WORKER,
                        "Connection established to peer {} with address {}.",
                        peer_id,
                        address
                    );
                }
                ConnectedPoint::Listener { send_back_addr, .. } => {
                    info!(
                        TARGET_WORKER,
                        "Connection established from address {}.",
                        send_back_addr,
                    );
                }
            },
            SwarmEvent::OutgoingConnectionError {
                error,
                peer_id: Some(peer_id),
                ..
            } => {
                let count =
                    self.try_to_send_message.entry(peer_id).or_default();
                let (retry, new_address) =
                    Self::dial_error_manager(error, peer_id, *count);

                if *count >= 3 {
                    self.clear_pending_messages(&peer_id);
                } else if retry {
                    let addresses = if !new_address.is_empty() {
                        new_address
                    } else if let Some(address) =
                        self.pending_dial.get(&peer_id)
                    {
                        address.clone()
                    } else {
                        vec![]
                    };

                    *count += 1;

                    if self
                        .swarm
                        .dial(
                            DialOpts::peer_id(peer_id)
                                .addresses(addresses.clone())
                                .build(),
                        )
                        .is_err()
                    {
                        self.swarm.behaviour_mut().discover(&peer_id);
                    };
                } else {
                    self.swarm.behaviour_mut().discover(&peer_id);
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                cause,
                ..
            } => {
                self.peer_identify.remove(&peer_id);
                self.pending_inbound_messages.remove(&peer_id);
                self.ephemeral_responses.remove(&peer_id);

                if let Some(count) = self.try_to_send_message.get_mut(&peer_id)
                {
                    *count += 1;

                    if *count >= 3 {
                        self.clear_pending_messages(&peer_id);
                    } else {
                        let addresses = self
                            .pending_dial
                            .get(&peer_id)
                            .cloned()
                            .unwrap_or_default();
                        if self
                            .swarm
                            .dial(
                                DialOpts::peer_id(peer_id)
                                    .addresses(addresses.clone())
                                    .build(),
                            )
                            .is_err()
                        {
                            self.swarm.behaviour_mut().discover(&peer_id);
                        };
                    }
                }

                if let Some(ConnectionError::IO(error)) = cause {
                    let endpoint = match endpoint {
                        ConnectedPoint::Dialer {
                            address,
                            role_override,
                            port_use,
                        } => {
                            let role: String = match role_override {
                                libp2p::core::Endpoint::Dialer => {
                                    "Dialer".to_owned()
                                }
                                libp2p::core::Endpoint::Listener => {
                                    "Listener".to_owned()
                                }
                            };

                            let port_use = match port_use {
                                libp2p::core::transport::PortUse::New => {
                                    "New".to_owned()
                                }
                                libp2p::core::transport::PortUse::Reuse => {
                                    "Reuse".to_owned()
                                }
                            };

                            format!(
                                "We make a connection to a node, its address is {}, within the connection our role is {}, the port used is {}",
                                address, role, port_use
                            )
                        }
                        ConnectedPoint::Listener {
                            local_addr,
                            send_back_addr,
                        } => format!(
                            "We received a connection, Our node was listening on address {} and the other node connected to us from address {}",
                            local_addr, send_back_addr
                        ),
                    };

                    error!(
                        TARGET_WORKER,
                        "A connection has been closed due to a problem, peer-id: {}, endpoint: {}, error: {}",
                        peer_id,
                        endpoint,
                        error
                    )
                }
            }
            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
                ..
            } => {
                error!(
                    TARGET_WORKER,
                    "A problem occurred during the handshake, our address is {} and the other node's is {}: {}",
                    local_addr,
                    send_back_addr,
                    error
                );
                //todo!()
            }
            SwarmEvent::ExpiredListenAddr { address, .. } => {
                warn!(
                    TARGET_WORKER,
                    "Listening address {} is no longer available", address
                );

                //todo!()
            }
            SwarmEvent::ListenerError { error, .. } => {
                error!(TARGET_WORKER, "ListenerError, {}", error);
                //  todo!()
            }
            SwarmEvent::IncomingConnection { .. }
            | SwarmEvent::ListenerClosed { .. }
            | SwarmEvent::Dialing { .. }
            | SwarmEvent::NewExternalAddrCandidate { .. }
            | SwarmEvent::ExternalAddrConfirmed { .. }
            | SwarmEvent::ExternalAddrExpired { .. }
            | SwarmEvent::NewExternalAddrOfPeer { .. }
            | SwarmEvent::NewListenAddr { .. } => {
                // We are not interested in this event at the moment.
            }
            _ => {
                warn!(TARGET_WORKER, "Unmatched event type {:?}", event)
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::routing::RoutingNode;

    use super::*;
    use serde::Deserialize;
    use test_log::test;

    use identity::keys::KeyPair;

    use serial_test::serial;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Dummy;

    // Build a relay server.
    fn build_worker(
        boot_nodes: Vec<RoutingNode>,
        random_walk: bool,
        node_type: NodeType,
        token: CancellationToken,
        memory_addr: String,
    ) -> NetworkWorker<Dummy> {
        let config = create_config(
            boot_nodes,
            random_walk,
            node_type,
            vec![memory_addr],
        );
        let keys = KeyPair::default();
        let mut registry = Registry::default();

        NetworkWorker::new(
            &mut registry,
            keys,
            config,
            None,
            KeyDerivator::Ed25519,
            token,
        )
        .unwrap()
    }

    // Create a config
    fn create_config(
        boot_nodes: Vec<RoutingNode>,
        random_walk: bool,
        node_type: NodeType,
        listen_addresses: Vec<String>,
    ) -> Config {
        let config = crate::routing::Config::new()
            .with_allow_non_globals_address_in_dht(true)
            .with_discovery_limit(50)
            .with_dht_random_walk(random_walk);

        Config {
            boot_nodes,
            node_type,
            tell: Default::default(),
            req_res: Default::default(),
            routing: config,
            external_addresses: vec![],
            listen_addresses,
            control_list: Default::default(),
        }
    }

    #[test(tokio::test)]
    #[serial]
    async fn test_no_boot_nodes() {
        let boot_nodes = vec![];
        let token = CancellationToken::new();

        // Build a node.
        let node_addr = "/memory/3000";
        let mut node = build_worker(
            boot_nodes.clone(),
            false,
            NodeType::Addressable,
            token.clone(),
            node_addr.to_owned(),
        );
        if let Err(e) = node.run_connection().await {
            assert_eq!(
                e.to_string(),
                "Network error: Can't connect to kore network"
            );
        };

        assert_eq!(node.state, NetworkState::Disconnected);
    }

    #[test(tokio::test)]
    #[serial]
    async fn test_fake_boot_node() {
        let mut boot_nodes = vec![];
        let token = CancellationToken::new();

        // Build a fake bootstrap node.
        let fake_boot_peer = PeerId::random();
        let fake_boot_addr = "/memory/3001";
        let fake_node = RoutingNode {
            peer_id: fake_boot_peer.to_string(),
            address: vec![fake_boot_addr.to_owned()],
        };
        boot_nodes.push(fake_node);

        // Build a node.
        let node_addr = "/memory/3002";
        let mut node = build_worker(
            boot_nodes.clone(),
            false,
            NodeType::Addressable,
            token.clone(),
            node_addr.to_owned(),
        );

        if let Err(e) = node.run_connection().await {
            assert_eq!(
                e.to_string(),
                "Network error: Can't connect to kore network"
            );
        };

        assert_eq!(node.state, NetworkState::Disconnected);
    }

    #[test(tokio::test)]
    #[serial]
    async fn test_connect() {
        let mut boot_nodes = vec![];

        let token = CancellationToken::new();

        // Build a bootstrap node.
        let boot_addr = "/memory/3003";
        let mut boot = build_worker(
            boot_nodes.clone(),
            false,
            NodeType::Bootstrap,
            token.clone(),
            boot_addr.to_owned(),
        );

        let boot_node = RoutingNode {
            peer_id: boot.local_peer_id().to_string(),
            address: vec![boot_addr.to_owned()],
        };

        boot_nodes.push(boot_node);

        // Build a node.
        let node_addr = "/memory/3004";
        let mut node = build_worker(
            boot_nodes,
            false,
            NodeType::Ephemeral,
            token.clone(),
            node_addr.to_owned(),
        );

        // Spawn the boot node
        tokio::spawn(async move {
            boot.run_main().await;
        });

        // Wait for connection.
        node.run_connection().await.unwrap();
    }
}
