// Copyright 2024 Antonio Estévez
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Network package.

#![warn(missing_docs)]

mod behaviour;
mod control_list;
pub mod error;
mod node;
mod routing;
mod service;
mod transport;
mod utils;
mod worker;

pub use control_list::Config as ControlListConfig;
pub use error::Error;
pub use libp2p::PeerId;
pub use routing::{Config as RoutingConfig, RoutingNode};
pub use service::NetworkService;
pub use tell::Config as TellConfig;
pub use worker::{NetworkError, NetworkState, NetworkWorker};

use serde::{Deserialize, Serialize};

/// The maximum allowed number of established connections per peer.
///
/// Typically, and by design of the network behaviours in this crate,
/// there is a single established connection per peer. However, to
/// avoid unnecessary and nondeterministic connection closure in
/// case of (possibly repeated) simultaneous dialing attempts between
/// two peers, the per-peer connection limit is not set to 1 but 2.
const MAX_CONNECTIONS_PER_PEER: usize = 2;

/// The network configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// The user agent.
    user_agent: String,

    /// The node type.
    node_type: NodeType,

    /// Listen addresses.
    listen_addresses: Vec<String>,

    /// External addresses.
    external_addresses: Vec<String>,

    /// Message telling configuration.
    tell: tell::Config,

    /// Routing configuration.
    routing: routing::Config,

    /// Configures port reuse for local sockets, which implies reuse of listening ports for outgoing connections to enhance NAT traversal capabilities.
    port_reuse: bool,

    /// Control List configuration.
    control_list: control_list::Config,
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
    /// Sets the user agent.
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = user_agent;
        self
    }
    /// Sets the node type.
    pub fn with_node_type(mut self, node_type: NodeType) -> Self {
        self.node_type = node_type;
        self
    }
    /// Sets the listen addresses.
    pub fn with_listen_addresses(mut self, listen_addresses: Vec<String>) -> Self {
        self.listen_addresses = listen_addresses;
        self
    }
    /// Sets the external addresses.
    pub fn with_external_addresses(mut self, external_addresses: Vec<String>) -> Self {
        self.external_addresses = external_addresses;
        self
    }
    /// Sets the message telling configuration.
    pub fn with_tell(mut self, tell: tell::Config) -> Self {
        self.tell = tell;
        self
    }
    /// Sets the routing configuration.
    pub fn with_routing(mut self, routing: routing::Config) -> Self {
        self.routing = routing;
        self
    }
    /// Sets the port reuse configuration.
    pub fn with_port_reuse(mut self, port_reuse: bool) -> Self {
        self.port_reuse = port_reuse;
        self
    }
    /// Sets the control list configuration.
    pub fn with_control_list(mut self, control_list: control_list::Config) -> Self {
        self.control_list = control_list;
        self
    }
    /// Returns the user agent.
    pub fn get_user_agent(&self) -> &String {
        &self.user_agent
    }
    /// Returns the node type.
    pub fn get_node_type(&self) -> NodeType {
        self.node_type.clone()
    }
    /// Returns the listen addresses.
    pub fn get_listen_addresses(&self) -> &Vec<String> {
        &self.listen_addresses
    }
    /// Returns the external addresses.
    pub fn get_external_addresses(&self) -> &Vec<String> {
        &self.external_addresses
    }
    /// Returns the message telling configuration.
    pub fn get_tell(&self) -> &tell::Config {
        &self.tell
    }
    /// Returns the routing configuration.
    pub fn get_routing(&self) -> &routing::Config {
        &self.routing
    }
    /// Returns the port reuse configuration.
    pub fn get_port_reuse(&self) -> bool {
        self.port_reuse
    }
    /// Returns the control list configuration.
    pub fn get_control_list(&self) -> &control_list::Config {
        &self.control_list
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
#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    /// Start providing the given keys.
    StartProviding {
        /// The keys to provide.
        keys: Vec<String>,
    },
    /// Send a message to the given peer.
    SendMessage {
        /// The peer to send the message to.
        peer: Vec<u8>,
        /// The message to send.
        message: Vec<u8>,
    },
    /// Bootstrap the network.
    Bootstrap,
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
