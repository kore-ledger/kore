//

use crate::error::Error;
use super::{ClientMessage, ClientResponse};

use identity::identifier::{KeyIdentifier, derive::KeyDerivator};
use network::{
    Command as NetworkCommand, 
    CommandHelper, 
    PeerId, 
    PublicKey,
    PublicKeyEd25519, 
    PublicKeysecp256k1,
};
use kore_common::{ActorMessage, NetworkMessage, RequestHandlerResponse};

use tokio::sync::{mpsc, oneshot, RwLock};
use tokio_util::sync::CancellationToken;
use serde::Deserialize;
use rmp_serde::Deserializer;
use tracing::error;

use std::{
    sync::Arc,
    io::Cursor,
    collections::HashMap,
};

#[derive(Clone)]
/// The Kore Client Service.
pub struct ClientService {
    network_sender: mpsc::Sender<NetworkCommand>,
    cancellation_token: CancellationToken,
    pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<ClientResponse>>>>,
}

impl ClientService {
    /// Create a new Client Service.
    pub fn build(
        network_sender: mpsc::Sender<NetworkCommand>,
        api_receiver: mpsc::Receiver<ClientMessage>,
        cancellation_token: CancellationToken,
    ) -> mpsc::Sender<CommandHelper<NetworkMessage>> {
        // Client service channel.
        let (client_sender, client_receiver) = mpsc::channel(100000);

        Self {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        }
        .run(client_receiver, api_receiver);
        client_sender
    }

    fn run(
        &mut self, 
        mut client_receiver: mpsc::Receiver<CommandHelper<NetworkMessage>>,
        mut api_receiver: mpsc::Receiver<ClientMessage>,
    ){
        let client_service = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(command) = client_receiver.recv() => {
                        // Handle the command.
                        if let Err(error) = client_service.handle_network_command(command).await {
                            error!("Error handling command: {}", error);
                        }
                    }
                    Some(message) = api_receiver.recv() => {
                        // Handle the API message.
                        if let Err(error) = client_service.handle_api_command(message).await {
                            error!("Error handling API message: {}", error);
                        }
                    }
                    _ = client_service.cancellation_token.cancelled() => {
                        break;
                    }
                }
            }
        });
    }

    async fn handle_network_command(&self, command: CommandHelper<NetworkMessage>) -> Result<(), Error> {
        match command {
            CommandHelper::ReceivedMessage { message } => {
                // Deserialize message
                let cur = Cursor::new(message);
                let mut de = Deserializer::new(cur);
                let message: NetworkMessage =
                    match Deserialize::deserialize(&mut de) {
                        Ok(message) => message,
                        Err(e) => {
                            return Err(Error::Network(format!(
                                "Can not deserialize message: {}",
                                e
                            )));
                        }
                    };

                // Handle message
                match message.message {
                    ActorMessage::EventRes { response } => {
                        // Get the sender for the response
                        let sender = {
                            let mut pending = self.pending_responses.write().await;
                            pending.remove(&message.info.request_id)
                        }.ok_or(Error::Network("Can't get response sender".to_owned()))?;
                        let client_response = match response {
                            RequestHandlerResponse::Ok(data) => ClientResponse::Successful(data),
                            RequestHandlerResponse::Response(err) => ClientResponse::Failed(err.to_owned()),
                            RequestHandlerResponse::None => ClientResponse::Failed("No response data".to_owned()),
                        };
                        sender.send(client_response)
                        .map_err(|response| Error::Network(format!(
                            "Can not send response: {}",
                            response
                        )))
                    }
                    _ => {
                        Err(Error::Network("Unsupported message type".to_owned()))
                    }
                }
            }
            _ => {
                Err(Error::Network("Unsupported command".to_owned()))
            }
        }
    }

    async fn handle_api_command(&self, command: ClientMessage) -> Result<(), Error> {
    
        let ClientMessage { 
            info, 
            request, 
            response_sender 
        } = command;

        let peer_id = self.peer_id(info.receiver.clone())?;
        let request_id = info.request_id.clone();
        let message = NetworkMessage {
            info,
            message: ActorMessage::EventReq { request },
        };
        
        // Message to Vec<u8>
        let msg =
            rmp_serde::to_vec(&message).map_err(|error| {
                Error::Network(format!("{}", error))
            })?;
        
        // Send message to network
        if let Err(error) = self
            .network_sender
            .send(NetworkCommand::SendMessage {
                peer: peer_id,
                message: msg,
            })
            .await
        {
            return Err(Error::Network(format!(
                "Can not send message to network: {}",
                error
            )));
        };

        // Store the response sender to be used when the response is received.
        {
            let mut pending = self.pending_responses.write().await;
            pending.insert(request_id, response_sender);
        }

        Ok(())
    }

    /// Get the PeerId from a KeyIdentifier.
    /// 
    /// # Arguments
    /// 
    /// * `identifier` - The KeyIdentifier to convert.
    /// 
    /// # Returns
    /// 
    /// A Result containing the PeerId or an Error.
    /// 
    fn peer_id(&self, identifier: KeyIdentifier) -> Result<PeerId, Error> {
        match identifier.derivator {
            KeyDerivator::Ed25519 => {
                let pk_bytes = identifier.public_key;
                let pk = PublicKeyEd25519::try_from_bytes(&pk_bytes)
                    .map_err(|_| Error::Network("Parse Ed25519 public key error".to_owned()))?;
                let public_key = PublicKey::from(pk);
                Ok(PeerId::from_public_key(&public_key))
            },
            KeyDerivator::Secp256k1 => {
                let pk_bytes = identifier.public_key;
                let pk = PublicKeysecp256k1::try_from_bytes(&pk_bytes)
                    .map_err(|_| Error::Network("Parse Secp256k1 public key error".to_owned()))?;
                let public_key = PublicKey::from(pk);
                Ok(PeerId::from_public_key(&public_key))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use identity::{
        identifier::{
            derive::KeyDerivator,
            derive::digest::DigestDerivator,
            DigestIdentifier,
        },
        keys::{KeyPair, KeyMaterial, KeyGenerator, Ed25519KeyPair, Secp256k1KeyPair},
    };
    use kore_common::{
        ComunicateInfo, EventRequest, CreateRequest, Signed, Signature, Namespace,
    };
    use tokio::sync::{mpsc, oneshot};
    use tokio_util::sync::CancellationToken;

    fn create_test_key_pair(derivator: KeyDerivator) -> KeyPair {
        match derivator {
            KeyDerivator::Ed25519 => {
                KeyPair::Ed25519(Ed25519KeyPair::from_seed(&[0u8; 32]))
            }
            KeyDerivator::Secp256k1 => {
                KeyPair::Secp256k1(Secp256k1KeyPair::from_seed(&[1u8; 32]))
            }
        }
    }

    fn create_test_key_identifier(derivator: KeyDerivator) -> KeyIdentifier {
        let keypair = create_test_key_pair(derivator);
        KeyIdentifier::new(
            keypair.get_key_derivator(),
            &keypair.public_key_bytes(),
        )
    }

    fn create_test_signed_request() -> Signed<EventRequest> {
        let keypair = create_test_key_pair(KeyDerivator::Ed25519);
        let mut ns = Namespace::new();
        ns.add("test");
        
        let create_request = CreateRequest {
            name: Some("Test Subject".to_string()),
            description: Some("Test description".to_string()),
            governance_id: DigestIdentifier::default(),
            schema_id: "schema-id".to_string(),
            namespace: ns,
        };

        let event_request = EventRequest::Create(create_request);
        let signature = Signature::new(&event_request, &keypair, DigestDerivator::Blake3_256)
            .expect("Failed to create signature");

        Signed::new(event_request, signature)
    }

    #[tokio::test]
    async fn test_peer_id_from_ed25519_identifier() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        let identifier = create_test_key_identifier(KeyDerivator::Ed25519);
        let result = service.peer_id(identifier);

        assert!(result.is_ok(), "Should convert Ed25519 identifier to PeerId");
    }

    #[tokio::test]
    async fn test_peer_id_from_secp256k1_identifier() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        let identifier = create_test_key_identifier(KeyDerivator::Secp256k1);
        let result = service.peer_id(identifier);

        assert!(result.is_ok(), "Should convert Secp256k1 identifier to PeerId");
    }

    #[tokio::test]
    async fn test_peer_id_invalid_ed25519_key() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        // Create invalid identifier with wrong key size
        let invalid_identifier = KeyIdentifier {
            derivator: KeyDerivator::Ed25519,
            public_key: vec![0u8; 10], // Invalid size for Ed25519
        };

        let result = service.peer_id(invalid_identifier);
        assert!(result.is_err(), "Should fail with invalid Ed25519 key");
        assert!(matches!(result.unwrap_err(), Error::Network(_)));
    }

    #[tokio::test]
    async fn test_peer_id_invalid_secp256k1_key() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        // Create invalid identifier with wrong key size
        let invalid_identifier = KeyIdentifier {
            derivator: KeyDerivator::Secp256k1,
            public_key: vec![0u8; 10], // Invalid size for Secp256k1
        };

        let result = service.peer_id(invalid_identifier);
        assert!(result.is_err(), "Should fail with invalid Secp256k1 key");
        assert!(matches!(result.unwrap_err(), Error::Network(_)));
    }

    #[tokio::test]
    async fn test_client_service_clone() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender: network_sender.clone(),
            cancellation_token: cancellation_token.clone(),
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        let cloned_service = service.clone();

        // Verify that the clone has the same internal state
        assert_eq!(
            Arc::strong_count(&service.pending_responses),
            Arc::strong_count(&cloned_service.pending_responses)
        );
    }

    #[tokio::test]
    async fn test_pending_responses_storage() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        let request_id = "test-request-123".to_string();
        let (_sender, _receiver) = oneshot::channel::<ClientResponse>();

        // Verify initially empty
        {
            let pending = service.pending_responses.read().await;
            assert_eq!(pending.len(), 0);
        }

        // Add a pending response
        {
            let (sender, _) = oneshot::channel::<ClientResponse>();
            let mut pending = service.pending_responses.write().await;
            pending.insert(request_id.clone(), sender);
            assert_eq!(pending.len(), 1);
        }

        // Verify it was stored
        {
            let pending = service.pending_responses.read().await;
            assert!(pending.contains_key(&request_id));
        }

        // Remove it
        {
            let mut pending = service.pending_responses.write().await;
            pending.remove(&request_id);
            assert_eq!(pending.len(), 0);
        }
    }

    #[tokio::test]
    async fn test_handle_api_command_serialization() {
        let (network_sender, mut network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        let sender_id = create_test_key_identifier(KeyDerivator::Ed25519);
        let receiver_id = create_test_key_identifier(KeyDerivator::Ed25519);

        let (response_sender, _response_receiver) = oneshot::channel();

        let client_message = ClientMessage {
            info: ComunicateInfo {
                request_id: "test-request-456".to_string(),
                version: 0,
                sender: sender_id,
                receiver: receiver_id.clone(),
                receiver_actor: "event_handler".to_string(),
            },
            request: create_test_signed_request(),
            response_sender,
        };

        // Spawn task to handle the network command
        tokio::spawn(async move {
            if let Some(NetworkCommand::SendMessage { peer: _, message: _ }) =
                network_receiver.recv().await
            {
                // Message was sent successfully
            }
        });

        let result = service.handle_api_command(client_message).await;
        
        // Should succeed in sending the message
        assert!(result.is_ok(), "Should successfully handle API command");

        // Verify pending response was stored
        let pending = service.pending_responses.read().await;
        assert_eq!(pending.len(), 1);
        assert!(pending.contains_key("test-request-456"));
    }

    #[tokio::test]
    async fn test_client_service_build() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let (_api_sender, api_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let helper_sender = ClientService::build(
            network_sender,
            api_receiver,
            cancellation_token,
        );

        // Verify the helper sender was created
        assert!(!helper_sender.is_closed());
    }

    #[tokio::test]
    async fn test_different_key_derivators() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        // Test Ed25519
        let ed25519_id = create_test_key_identifier(KeyDerivator::Ed25519);
        let ed25519_result = service.peer_id(ed25519_id);
        assert!(ed25519_result.is_ok());

        // Test Secp256k1
        let secp256k1_id = create_test_key_identifier(KeyDerivator::Secp256k1);
        let secp256k1_result = service.peer_id(secp256k1_id);
        assert!(secp256k1_result.is_ok());

        // Verify different PeerIds
        assert_ne!(
            ed25519_result.unwrap(),
            secp256k1_result.unwrap(),
            "Different key types should produce different PeerIds"
        );
    }

    #[tokio::test]
    async fn test_handle_api_command_network_failure() {
        let (network_sender, _network_receiver) = mpsc::channel(1);
        let cancellation_token = CancellationToken::new();

        // Drop the receiver to simulate network failure
        drop(_network_receiver);

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        let sender_id = create_test_key_identifier(KeyDerivator::Ed25519);
        let receiver_id = create_test_key_identifier(KeyDerivator::Ed25519);

        let (response_sender, _response_receiver) = oneshot::channel();

        let client_message = ClientMessage {
            info: ComunicateInfo {
                request_id: "test-request-789".to_string(),
                version: 0,
                sender: sender_id,
                receiver: receiver_id,
                receiver_actor: "event_handler".to_string(),
            },
            request: create_test_signed_request(),
            response_sender,
        };

        let result = service.handle_api_command(client_message).await;
        
        // Should fail because network receiver is dropped
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Network(_)));
    }

    #[tokio::test]
    async fn test_multiple_pending_responses() {
        let (network_sender, _network_receiver) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let service = ClientService {
            network_sender,
            cancellation_token,
            pending_responses: Arc::new(RwLock::new(HashMap::new())),
        };

        // Add multiple pending responses
        {
            let mut pending = service.pending_responses.write().await;
            for i in 0..5 {
                let (sender, _) = oneshot::channel::<ClientResponse>();
                pending.insert(format!("request-{}", i), sender);
            }
            assert_eq!(pending.len(), 5);
        }

        // Verify all are stored
        {
            let pending = service.pending_responses.read().await;
            for i in 0..5 {
                assert!(pending.contains_key(&format!("request-{}", i)));
            }
        }

        // Remove some
        {
            let mut pending = service.pending_responses.write().await;
            pending.remove("request-2");
            pending.remove("request-4");
            assert_eq!(pending.len(), 3);
        }
    }
}