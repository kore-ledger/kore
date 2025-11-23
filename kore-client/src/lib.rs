//

mod config;
mod error;
mod service;

pub use config::Config;
pub use error::Error;

use identity::{keys::{KeyPair, KeyMaterial}, identifier::{KeyIdentifier, derive::KeyDerivator}};
use network::{NetworkWorker, CommandHelper};
use kore_common::{ActorMessage, EventRequest, NetworkMessage, ComunicateInfo, Signed};

use tokio::sync::{mpsc::Sender, oneshot};
use tokio_util::sync::CancellationToken;
use prometheus_client::registry::Registry;

use std::{
    sync::atomic::{AtomicU64, Ordering},
};

/// The Kore Client API.
/// 
pub struct Api {
    /// The client's identifier.
    client_id: KeyIdentifier,
    /// The network configuration.
    pub config: Config,
    /// The request counter.
    request_counter: AtomicU64,
    /// The client service sender.
    client_service: Sender<CommandHelper<NetworkMessage>>,
}

impl Api {
    /// Create a new API instance.
    pub async fn build(
        keys: KeyPair,
        config: Config,
        registry: &mut Registry,
        cancellation_token: CancellationToken,
    ) -> Result<Self, Error> {

        let client_id = match &keys {
            KeyPair::Ed25519(key_pair) => {
                KeyIdentifier::new(
                    KeyDerivator::Ed25519,
                    &key_pair.public_key_bytes(),
                    
                )
            }
            KeyPair::Secp256k1(key_pair) => {
                KeyIdentifier::new(
                    KeyDerivator::Secp256k1,
                    &key_pair.public_key_bytes(),

                )
            }
        };

        // Create the network worker.
        let mut network_worker = NetworkWorker::<NetworkMessage>::new(
            registry,
            keys,
            config.network_config.clone(),
            None,
            KeyDerivator::Ed25519,
            cancellation_token.clone()
        ).map_err(|e| Error::Network(format!("Can't create network worker: {}", e)))?;

        // Get the local peer identifier.
        let local_peer_id = network_worker.local_peer_id();

        // Create the cliente service.
        let client_service = service::ClientService::build(
            local_peer_id,
            network_worker.service().sender(),
            cancellation_token.clone(),
        );

        // Add command helper to the network worker.
        network_worker.add_helper_sender(client_service.clone());

        Ok(Self { client_id, config, request_counter: AtomicU64::new(0), client_service })
    }

    /// Send an event request to a peer.
    /// 
    /// # Arguments
    /// 
    /// * `receiver` - The receiver's key identifier.
    /// * `request` - The signed event request.
    /// 
    /// # Returns
    /// 
    /// A result indicating success or failure.
    /// 
    pub async fn send_request(
        &self, 
        receiver: KeyIdentifier,
        request: Signed<EventRequest>
    ) -> Result<(), Error> {
        let info = ComunicateInfo {
            request_id: self.next_request_id(),
            version: 0,
            sender: self.client_id.clone(),
            receiver,
            receiver_actor: "event_handler".to_string(),
        };
        let message = NetworkMessage {
            info,
            message: ActorMessage::EventReq { request },
        };

        self.client_service.send(
            CommandHelper::SendMessage { message }
        )
        .await
        .map_err(|e| Error::Network(format!("Can't send request: {}", e)))?;
        
        Ok(())
    }

    /// Get the next unique request identifier.
    /// 
    /// # Returns
    /// 
    /// A unique request identifier.
    /// 
    pub fn next_request_id(&self) -> String {
        let id =self.request_counter.fetch_add(1, Ordering::SeqCst);
        format!("{}", id)
    }
}

/// A struct representing a client message.
pub struct ClientMessage {
    pub info: ComunicateInfo,
    pub request: Signed<EventRequest>,
    pub response_sender: oneshot::Sender<ClientResponse>,
}

pub enum ClientResponse {
    Successful,
    Failed(String),
}