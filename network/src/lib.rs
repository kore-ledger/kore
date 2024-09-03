// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Network package.

#![warn(missing_docs)]

mod behaviour;
mod control_list;
pub mod error;
//mod node;
mod routing;
mod service;
mod transport;
mod utils;
mod worker;

use std::fmt::Debug;

pub use control_list::Config as ControlListConfig;
pub use error::Error;
use identity::identifier::KeyIdentifier;
pub use libp2p::{
    identity::{
        ed25519::PublicKey as PublicKeyEd25519,
        secp256k1::PublicKey as PublicKeysecp256k1, PublicKey,
    },
    PeerId,
};
pub use routing::{Config as RoutingConfig, RoutingNode};
pub use service::NetworkService;
pub use tell::Config as TellConfig;
pub use worker::{NetworkError, NetworkState, NetworkWorker};

use serde::{Deserialize, Serialize};

/// The network configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// The user agent.
    pub user_agent: String,

    /// The node type.
    pub node_type: NodeType,

    /// Listen addresses.
    pub listen_addresses: Vec<String>,

    /// External addresses.
    pub external_addresses: Vec<String>,

    /// Message telling configuration.
    pub tell: tell::Config,

    /// Routing configuration.
    pub routing: routing::Config,

    /// Configures port reuse for local sockets, which implies reuse of listening ports for outgoing connections to enhance NAT traversal capabilities.
    pub port_reuse: bool,

    /// Control List configuration.
    pub control_list: control_list::Config,
}

impl Config {
    /// Create a new configuration.
    pub fn new(
        node_type: NodeType,
        listen_addresses: Vec<String>,
        external_addresses: Vec<String>,
        boot_nodes: Vec<RoutingNode>,
        port_reuse: bool,
    ) -> Self {
        Self {
            user_agent: "kore-node".to_owned(),
            node_type,
            listen_addresses,
            external_addresses,
            tell: tell::Config::default(),
            routing: routing::Config::new(boot_nodes),
            port_reuse,
            control_list: control_list::Config::default(),
        }
    }
}

/// Type of a node.
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub enum NodeType {
    /// Bootstrap node.
    Bootstrap,
    /// Addressable node.
    #[default]
    Addressable,
    /// Ephemeral node.
    Ephemeral,
}

/// Command enumeration for the network service.
#[derive(Debug)]
pub enum Command {
    /// Send a message to the given peer.
    SendMessage {
        /// The peer to send the message to.
        peer: PeerId,
        /// The message to send.
        message: Vec<u8>,
    },
}

/// Event enumeration for the network service.
#[derive(Debug, Serialize, Deserialize)]
pub enum Event {
    /// Connected to a bootstrap node.
    ConnectedToBootstrap {
        /// The peer ID of the bootstrap node.
        peer: String,
    },

    /// A message was received.
    MessageReceived {
        /// The peer that sent the message.
        peer: String,
        /// The message.
        message: Vec<u8>,
    },

    /// A message was sent.
    MessageSent {
        /// The peer that the message was sent to.
        peer: String,
    },

    /// A peer was identified.
    PeerIdentified {
        /// The peer ID.
        peer: String,
        /// The peer's address.
        addresses: Vec<String>,
    },

    /// Network state changed.
    StateChanged(worker::NetworkState),

    /// Network error.
    Error(Error),
}

/// Command enumeration for the Helper service.
#[derive(Debug, Serialize, Deserialize)]
pub enum CommandHelper<T>
where
    T: Debug + Serialize,
{
    /// Send a message to the given peer.
    SendMessage {
        /// The message to send.
        message: T,
    },
    /// Received a message.
    ReceivedMessage {
        /// The message received.
        message: Vec<u8>,
    },
}

/// Event enumeration for the Helper service.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComunicateInfo {
    /// The request id.
    pub request_id: String,
    /// The sender key identifier.
    pub sender: KeyIdentifier,
    /// The receiver key identifier.
    pub reciver: KeyIdentifier,
    /// The receiver actor.
    pub reciver_actor: String,
    /// schema of subject
    pub schema: String,
}
