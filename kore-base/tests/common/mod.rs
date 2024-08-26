use actor::SystemRef;
use identity::{
    identifier::{
        derive::{digest::DigestDerivator, KeyDerivator},
        DigestIdentifier, KeyIdentifier,
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair},
};
use kore_base::{
    service::HelperService, system, Config as ConfigKoreBase, EventRequest, FactRequest, KoreBaseConfig, NetworkMessage, Signature, Signed, StartRequest, ValueWrapper
};
use network::{CommandHelper, Event as NetworkEvent, PeerId, PublicKey, PublicKeyEd25519, RoutingConfig};
use network::{Config, NetworkWorker, NodeType, RoutingNode};
use prometheus_client::registry::Registry;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Receiver};
use tokio_util::sync::CancellationToken;

pub async fn create_system() -> SystemRef {
    let dir = tempfile::tempdir().expect("Can not create temporal directory.");
    let path = dir.path().to_str().unwrap().to_owned();
    let config = KoreBaseConfig::new(&path);
    system(config, "password").await.unwrap()
}

// Create governance request mock.
pub fn create_start_request_mock(
    key_pair: KeyPair,
    key_identifier: KeyIdentifier,
) -> Signed<EventRequest> {
    let req = StartRequest {
        governance_id: DigestIdentifier::default(),
        schema_id: "governance".to_string(),
        namespace: "namespace".to_string(),
        name: "name".to_string(),
        public_key: key_identifier.clone(),
    };
    let content = EventRequest::Create(req);
    let signature =
        Signature::new(&content, &key_pair, DigestDerivator::SHA2_256).unwrap();
    Signed { content, signature }
}

pub fn fact_request_mock(
    key_pair: KeyPair,
    subject_id: DigestIdentifier,
    patch: ValueWrapper,
) -> Signed<EventRequest> {
    let req = FactRequest {
        subject_id: subject_id.clone(),
        payload: patch,
    };

    let bytes = bincode::serialize(&req).unwrap();
    
    let content = EventRequest::Fact(req);
    let signature =
        Signature::new(&content, &key_pair, DigestDerivator::SHA2_256).unwrap();

    
    Signed { content, signature }
}
// Mokcs
#[allow(dead_code)]
pub fn issuer_identity(name: &str) -> (KeyPair, KeyIdentifier) {
    let filler = [0u8; 32];
    let mut value = name.as_bytes().to_vec();
    value.extend(filler.iter());
    value.truncate(32);
    let kp = Ed25519KeyPair::from_secret_key(&value);
    let id = KeyIdentifier::new(KeyDerivator::Ed25519, &kp.public_key_bytes());
    (KeyPair::Ed25519(kp), id)
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Dummy {}

// Build a relay server.
pub fn build_worker(
    boot_nodes: Vec<RoutingNode>,
    node_type: NodeType,
    token: CancellationToken,
    tcp_addr: Option<String>,
    keys: KeyPair,
) -> (NetworkWorker<NetworkMessage>, Receiver<NetworkEvent>) {
    let listen_addresses = if let Some(addr) = tcp_addr {
        vec![addr]
    } else {
        vec![]
    };
    let config = create_config(
        boot_nodes,
        node_type,
        listen_addresses,
        false,
    );
    let mut registry = Registry::default();
    let (event_sender, event_receiver) = mpsc::channel(100);
    let worker = NetworkWorker::new(
        &mut registry,
        keys,
        config,
        event_sender.clone(),
        KeyDerivator::Ed25519,
        token,
    )
    .unwrap();
    (worker, event_receiver)
}

// Create a config
pub fn create_config(
    boot_nodes: Vec<RoutingNode>,
    node_type: NodeType,
    listen_addresses: Vec<String>,
    port_reuse: bool,
) -> Config {
    let config = RoutingConfig::new(boot_nodes.clone())
        .with_allow_non_globals_in_dht(true)
        .with_allow_private_ip(true)
        .with_discovery_limit(50)
        .with_mdns(false)
        .with_dht_random_walk(false);

    Config {
        user_agent: "kore::node".to_owned(),
        node_type,
        tell: Default::default(),
        routing: config,
        external_addresses: vec![],
        listen_addresses,
        port_reuse,
        control_list: Default::default(),
    }
}


pub fn get_peer_id(keys: KeyPair) -> PeerId {
    let public_key = PublicKeyEd25519::try_from_bytes(keys.key_identifier().public_key.as_slice()).unwrap();
    let peer = PublicKey::from(public_key);
    peer.to_peer_id()
}