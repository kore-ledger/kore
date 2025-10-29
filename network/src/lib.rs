//! # Network package.

#![warn(missing_docs)]

mod behaviour;
mod control_list;
pub mod error;
//mod node;
mod monitor;
mod routing;
mod service;
mod transport;
mod utils;
mod worker;

use std::fmt::{self, Debug};

pub use control_list::Config as ControlListConfig;
pub use error::Error;
use identity::identifier::KeyIdentifier;
pub use libp2p::{
    PeerId,
    identity::{
        PublicKey, ed25519::PublicKey as PublicKeyEd25519,
        secp256k1::PublicKey as PublicKeysecp256k1,
    },
};
pub use monitor::*;
pub use routing::{Config as RoutingConfig, RoutingNode};
pub use service::NetworkService;
pub use tell::Config as TellConfig;
pub use utils::NetworkState;
pub use worker::NetworkWorker;

use serde::{Deserialize, Serialize};

pub use crate::utils::ReqResConfig;

/// The network configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// The node type.
    pub node_type: NodeType,

    /// Listen addresses.
    pub listen_addresses: Vec<String>,

    /// External addresses.
    pub external_addresses: Vec<String>,

    /// Bootnodes to connect to.
    pub boot_nodes: Vec<RoutingNode>,

    /// Tell configuration.
    pub tell: tell::Config,

    /// ReqRes configuration.
    pub req_res: ReqResConfig,

    /// Routing configuration.
    pub routing: routing::Config,

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
    ) -> Self {
        Self {
            boot_nodes,
            node_type,
            listen_addresses,
            external_addresses,
            tell: tell::Config::default(),
            req_res: ReqResConfig::default(),
            routing: routing::Config::new(),
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

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeType::Bootstrap => write!(f, "Bootstrap"),
            NodeType::Addressable => write!(f, "Addressable"),
            NodeType::Ephemeral => write!(f, "Ephemeral"),
        }
    }
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Event {
    /// Network state changed.
    StateChanged(utils::NetworkState),

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
    /// The request version.
    pub version: u64,
    /// The sender key identifier.
    pub sender: KeyIdentifier,
    /// The receiver key identifier.
    pub reciver: KeyIdentifier,
    /// The receiver actor.
    pub reciver_actor: String,
}
