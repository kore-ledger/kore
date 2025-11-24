//

mod config;
mod error;
mod service;

pub use config::Config;
pub use error::Error;

use identity::{keys::{KeyPair, KeyMaterial}, identifier::{KeyIdentifier, derive::KeyDerivator}};
use network::NetworkWorker;
use kore_common::{EventRequest, NetworkMessage, ComunicateInfo, Signed, RequestData};

use serde::{Serialize, Deserialize};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use prometheus_client::registry::Registry;

use std::{
    fmt, sync::atomic::{AtomicU64, Ordering}
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
    /// The API service sender.
    api_sender: mpsc::Sender<ClientMessage>,
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

        let (api_sender, api_receiver) = mpsc::channel(10000);

        // Create the cliente service.
        let client_service = service::ClientService::build(
            network_worker.service().sender(),
            api_receiver,
            cancellation_token.clone(),
        );

        // Add command helper to the network worker.
        network_worker.add_helper_sender(client_service);

        Ok(Self { client_id, config, request_counter: AtomicU64::new(0), api_sender })
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
    ) -> Result<ClientResponse, Error> {
        // Create the communication info.
        let info = ComunicateInfo {
            request_id: self.next_request_id(),
            version: 0,
            sender: self.client_id.clone(),
            receiver,
            receiver_actor: "event_handler".to_string(),
        };

        // Create the response channel.
        let (response_sender, response_receiver) = oneshot::channel();

        self.api_sender.send(
            ClientMessage { info, request, response_sender }
        )
        .await
        .map_err(|e| Error::Network(format!("Can't send request: {}", e)))?;
        
        // Wait for the response.
        let response = response_receiver
            .await
            .map_err(|e| Error::Network(format!("Can't receive response: {}", e)))?;

        Ok(response)
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

/// A struct representing a client request.
pub struct ClientMessage {
    /// The communication information.
    pub info: ComunicateInfo,
    /// The signed event request.
    pub request: Signed<EventRequest>,
    /// The response sender.
    pub response_sender: oneshot::Sender<ClientResponse>,
}

/// A enum representing a client response.
#[derive(Serialize, Deserialize, Debug)]
pub enum ClientResponse {
    /// Indicates a successful response with the Kore Ledger request id.
    Successful(RequestData),
    /// Indicates a failed response with an error message.
    Failed(String),
}
    
impl fmt::Display for ClientResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientResponse::Successful(data) => write!(f, "Successful: {:?}", data),
            ClientResponse::Failed(err) => write!(f, "Failed: {}", err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use identity::{
        identifier::derive::KeyDerivator,
        keys::{KeyGenerator, Ed25519KeyPair, Secp256k1KeyPair},
    };

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

    #[test]
    fn test_next_request_id_increments() {
        let keypair = create_test_key_pair(KeyDerivator::Ed25519);
        let client_id = KeyIdentifier::new(
            keypair.get_key_derivator(),
            &keypair.public_key_bytes(),
        );
        
        let (api_sender, _) = mpsc::channel(100);
        let config = Config {
            db_path: "/tmp/test".to_string(),
            network_config: network::Config::default(),
        };

        let api = Api {
            client_id,
            config,
            request_counter: AtomicU64::new(0),
            api_sender,
        };

        let id1 = api.next_request_id();
        let id2 = api.next_request_id();
        let id3 = api.next_request_id();

        assert_eq!(id1, "0");
        assert_eq!(id2, "1");
        assert_eq!(id3, "2");
    }

    #[test]
    fn test_next_request_id_multiple_calls() {
        let keypair = create_test_key_pair(KeyDerivator::Ed25519);
        let client_id = KeyIdentifier::new(
            keypair.get_key_derivator(),
            &keypair.public_key_bytes(),
        );
        
        let (api_sender, _) = mpsc::channel(100);
        let config = Config {
            db_path: "/tmp/test".to_string(),
            network_config: network::Config::default(),
        };

        let api = Api {
            client_id,
            config,
            request_counter: AtomicU64::new(0),
            api_sender,
        };

        // Generate multiple IDs
        let mut ids = Vec::new();
        for _ in 0..10 {
            ids.push(api.next_request_id());
        }

        // Verify uniqueness and ordering
        for i in 0..10 {
            assert_eq!(ids[i], format!("{}", i));
        }
    }

    #[test]
    fn test_next_request_id_with_initial_value() {
        let keypair = create_test_key_pair(KeyDerivator::Ed25519);
        let client_id = KeyIdentifier::new(
            keypair.get_key_derivator(),
            &keypair.public_key_bytes(),
        );
        
        let (api_sender, _) = mpsc::channel(100);
        let config = Config {
            db_path: "/tmp/test".to_string(),
            network_config: network::Config::default(),
        };

        let api = Api {
            client_id,
            config,
            request_counter: AtomicU64::new(100),
            api_sender,
        };

        let id1 = api.next_request_id();
        let id2 = api.next_request_id();

        assert_eq!(id1, "100");
        assert_eq!(id2, "101");
    }

    #[test]
    fn test_client_response_successful_display() {
        let response = ClientResponse::Successful(RequestData {
            request_id: "req-123".to_string(),
            subject_id: "subj-456".to_string(),
        });

        let display = format!("{}", response);
        assert!(display.contains("Successful"));
        assert!(display.contains("req-123"));
        assert!(display.contains("subj-456"));
    }

    #[test]
    fn test_client_response_failed_display() {
        let response = ClientResponse::Failed("Connection error".to_string());

        let display = format!("{}", response);
        assert_eq!(display, "Failed: Connection error");
    }

    #[test]
    fn test_client_response_debug() {
        let successful = ClientResponse::Successful(RequestData {
            request_id: "req-1".to_string(),
            subject_id: "subj-1".to_string(),
        });

        let debug = format!("{:?}", successful);
        assert!(debug.contains("Successful"));

        let failed = ClientResponse::Failed("Error message".to_string());
        let debug_failed = format!("{:?}", failed);
        assert!(debug_failed.contains("Failed"));
        assert!(debug_failed.contains("Error message"));
    }

    #[test]
    fn test_client_response_serialization() {
        let successful = ClientResponse::Successful(RequestData {
            request_id: "req-123".to_string(),
            subject_id: "subj-456".to_string(),
        });

        let json = serde_json::to_string(&successful).unwrap();
        assert!(json.contains("Successful"));
        assert!(json.contains("req-123"));

        let failed = ClientResponse::Failed("Test error".to_string());
        let json_failed = serde_json::to_string(&failed).unwrap();
        assert!(json_failed.contains("Failed"));
        assert!(json_failed.contains("Test error"));
    }

    #[test]
    fn test_client_response_deserialization() {
        let json_successful = r#"{"Successful":{"request_id":"req-1","subject_id":"subj-1"}}"#;
        let response: ClientResponse = serde_json::from_str(json_successful).unwrap();
        
        match response {
            ClientResponse::Successful(data) => {
                assert_eq!(data.request_id, "req-1");
                assert_eq!(data.subject_id, "subj-1");
            }
            _ => panic!("Expected Successful variant"),
        }

        let json_failed = r#"{"Failed":"Error occurred"}"#;
        let response_failed: ClientResponse = serde_json::from_str(json_failed).unwrap();
        
        match response_failed {
            ClientResponse::Failed(msg) => {
                assert_eq!(msg, "Error occurred");
            }
            _ => panic!("Expected Failed variant"),
        }
    }

    #[test]
    fn test_client_response_pattern_matching() {
        let responses = vec![
            ClientResponse::Successful(RequestData {
                request_id: "1".to_string(),
                subject_id: "s1".to_string(),
            }),
            ClientResponse::Failed("error".to_string()),
        ];

        let mut successful_count = 0;
        let mut failed_count = 0;

        for response in responses {
            match response {
                ClientResponse::Successful(_) => successful_count += 1,
                ClientResponse::Failed(_) => failed_count += 1,
            }
        }

        assert_eq!(successful_count, 1);
        assert_eq!(failed_count, 1);
    }

    #[test]
    fn test_api_client_id_ed25519() {
        let keypair = create_test_key_pair(KeyDerivator::Ed25519);
        let expected_id = KeyIdentifier::new(
            KeyDerivator::Ed25519,
            &keypair.public_key_bytes(),
        );
        
        let (api_sender, _) = mpsc::channel(100);
        let config = Config {
            db_path: "/tmp/test".to_string(),
            network_config: network::Config::default(),
        };

        let api = Api {
            client_id: expected_id.clone(),
            config,
            request_counter: AtomicU64::new(0),
            api_sender,
        };

        assert_eq!(api.client_id.derivator, KeyDerivator::Ed25519);
        assert_eq!(api.client_id.public_key, expected_id.public_key);
    }

    #[test]
    fn test_api_client_id_secp256k1() {
        let keypair = create_test_key_pair(KeyDerivator::Secp256k1);
        let expected_id = KeyIdentifier::new(
            KeyDerivator::Secp256k1,
            &keypair.public_key_bytes(),
        );
        
        let (api_sender, _) = mpsc::channel(100);
        let config = Config {
            db_path: "/tmp/test".to_string(),
            network_config: network::Config::default(),
        };

        let api = Api {
            client_id: expected_id.clone(),
            config,
            request_counter: AtomicU64::new(0),
            api_sender,
        };

        assert_eq!(api.client_id.derivator, KeyDerivator::Secp256k1);
        assert_eq!(api.client_id.public_key, expected_id.public_key);
    }

    #[test]
    fn test_api_config_access() {
        let keypair = create_test_key_pair(KeyDerivator::Ed25519);
        let client_id = KeyIdentifier::new(
            keypair.get_key_derivator(),
            &keypair.public_key_bytes(),
        );
        
        let (api_sender, _) = mpsc::channel(100);
        let config = Config {
            db_path: "/var/lib/kore".to_string(),
            network_config: network::Config::default(),
        };

        let api = Api {
            client_id,
            config: config.clone(),
            request_counter: AtomicU64::new(0),
            api_sender,
        };

        assert_eq!(api.config.db_path, "/var/lib/kore");
    }

    #[test]
    fn test_client_response_with_empty_messages() {
        let failed_empty = ClientResponse::Failed(String::new());
        assert_eq!(format!("{}", failed_empty), "Failed: ");

        let successful = ClientResponse::Successful(RequestData {
            request_id: String::new(),
            subject_id: String::new(),
        });
        let display = format!("{}", successful);
        assert!(display.contains("Successful"));
    }

    #[test]
    fn test_request_counter_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let keypair = create_test_key_pair(KeyDerivator::Ed25519);
        let client_id = KeyIdentifier::new(
            keypair.get_key_derivator(),
            &keypair.public_key_bytes(),
        );
        
        let (api_sender, _) = mpsc::channel(100);
        let config = Config {
            db_path: "/tmp/test".to_string(),
            network_config: network::Config::default(),
        };

        let api = Arc::new(Api {
            client_id,
            config,
            request_counter: AtomicU64::new(0),
            api_sender,
        });

        let mut handles = vec![];

        // Spawn multiple threads that each generate IDs
        for _ in 0..5 {
            let api_clone = Arc::clone(&api);
            let handle = thread::spawn(move || {
                let mut ids = vec![];
                for _ in 0..10 {
                    ids.push(api_clone.next_request_id());
                }
                ids
            });
            handles.push(handle);
        }

        // Collect all IDs
        let mut all_ids = vec![];
        for handle in handles {
            let ids = handle.join().unwrap();
            all_ids.extend(ids);
        }

        // Verify we got 50 IDs total (5 threads * 10 IDs each)
        assert_eq!(all_ids.len(), 50);

        // Convert to numbers and verify all are unique
        let mut numbers: Vec<u64> = all_ids.iter()
            .map(|s| s.parse().unwrap())
            .collect();
        numbers.sort();
        numbers.dedup();
        
        assert_eq!(numbers.len(), 50, "All IDs should be unique");
    }
}
