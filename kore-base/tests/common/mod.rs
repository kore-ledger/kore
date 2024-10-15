/*
use std::collections::HashSet;

use actor::SystemRef;
use identity::{
    identifier::{
        derive::{digest::DigestDerivator, KeyDerivator},
        DigestIdentifier, KeyIdentifier,
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair},
};
use kore_base::{
    init_state, intermediary::Intermediary, system, Event, EventRequest,
    FactRequest, HashId, KoreBaseConfig, NetworkMessage, Node, Signature,
    Signed, StartRequest, Subject, SubjectState, ValidationInfo, ValueWrapper,
};
use network::{Config, NetworkWorker, NodeType, RoutingNode};
use network::{
    Event as NetworkEvent, PeerId, PublicKey, PublicKeyEd25519, RoutingConfig,
};
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
    signed(content, key_pair)
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
    let content = EventRequest::Fact(req);
    signed(content, key_pair)
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
    let config = create_config(boot_nodes, node_type, listen_addresses, false);
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
    let public_key = PublicKeyEd25519::try_from_bytes(
        keys.key_identifier().public_key.as_slice(),
    )
    .unwrap();
    let peer = PublicKey::from(public_key);
    peer.to_peer_id()
}

pub async fn create_network(
    system: SystemRef,
    addr: String,
    node_keys: KeyPair,
    boot_node: Vec<RoutingNode>,
) {
    // Initialize boot worker
    let (mut node1_worker, _boot_worker_receiver) = build_worker(
        boot_node,
        NodeType::Bootstrap,
        CancellationToken::new(),
        Some(addr),
        node_keys,
    );

    // Create worker
    let service = Intermediary::new(
        node1_worker.service().sender().clone(),
        KeyDerivator::Ed25519,
        system.clone(),
    );

    node1_worker.add_helper_sender(service.service().sender());
    system.add_helper("NetworkIntermediary", service).await;

    tokio::spawn(async move {
        let _ = node1_worker.run().await;
    });
}

pub async fn initilize_use_case() -> (Node, KeyPair, Subject, KeyPair) {
    let node_keys: KeyPair = KeyPair::Ed25519(Ed25519KeyPair::new());
    let node = Node::new(&node_keys).unwrap();

    let request = create_start_request_mock(
        node_keys.clone(),
        node_keys.key_identifier().clone(),
    );
    let gov_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
    let event = Event::from_create_request(
        &gov_keys,
        &request,
        0,
        &init_state(&node_keys.key_identifier().to_string()),
        DigestDerivator::Blake3_256,
    )
    .unwrap();

    let signed_event = signed(event, node_keys.clone());
    let subject = Subject::from_event(gov_keys.clone(), &signed_event).unwrap();

    (node, node_keys, subject, gov_keys)
}

pub fn create_info_gov_genesis_event(
    node_keys: KeyPair,
    gov_keys: KeyPair,
    subject_state: SubjectState,
) -> Signed<Event> {
    // Create governance genesis event
    let request = create_start_request_mock(
        gov_keys.clone(),
        subject_state.subject_key.clone(),
    );
    let event = Event::from_create_request(
        &gov_keys,
        &request,
        0,
        &init_state(&node_keys.key_identifier().to_string()),
        DigestDerivator::Blake3_256,
    )
    .unwrap();
    signed(event, gov_keys)
}

pub fn signed<T: borsh::de::BorshDeserialize + Clone + HashId>(
    event: T,
    keys: KeyPair,
) -> Signed<T> {
    let signature =
        Signature::new(&event, &keys, DigestDerivator::Blake3_256).unwrap();
    Signed {
        content: event,
        signature,
    }
}

pub fn create_info_gov_event(
    gov_keys: KeyPair,
    subject_id: DigestIdentifier,
    prev_event: ValidationInfo,
    value: ValueWrapper,
) -> Signed<Event> {
    let request: Signed<kore_base::EventRequest> =
        fact_request_mock(gov_keys.clone(), subject_id.clone(), value.clone());

    // Calculate event hash of first event
    let event_hash = prev_event
        .event
        .hash_id(DigestDerivator::Blake3_256)
        .unwrap();

    let fact_event = Event {
        subject_id: subject_id.clone(),
        event_request: request.clone(),
        sn: prev_event.event.content.sn + 1,
        gov_version: prev_event.event.content.gov_version,
        patch: value,
        state_hash: event_hash.clone(),
        eval_success: true,
        appr_required: false,
        approved: true,
        hash_prev_event: event_hash,
        evaluators: HashSet::default(),
        approvers: HashSet::default(),
    };
    signed(fact_event, gov_keys)
}
 */