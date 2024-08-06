use crate::common::{create_start_request_mock, create_system, init_state};
use identity::{
    identifier::derive::digest::DigestDerivator,
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{Event, Governance, Node, Signature, Signed, Subject};

#[tokio::test]
async fn test_validation() {
    let system = create_system().await;
    // Node
    let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
    let node = Node::new(&node_keys, DigestDerivator::Blake3_256).unwrap();
    let node_actor = system.create_root_actor("node", node).await.unwrap();
    // Governance
    let request = create_start_request_mock("issuer");
    let gov_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
    let event = Event::from_create_request(
        &gov_keys,
        &request,
        0,
        &init_state(),
        DigestDerivator::Blake3_256,
    )
    .unwrap();
    let signature =
        Signature::new(&event, &gov_keys, DigestDerivator::Blake3_256)
            .unwrap();
    let signed_event = Signed {
        content: event,
        signature,
    };
    let subject = Subject::from_event(
        gov_keys,
        DigestDerivator::Blake3_256,
        &signed_event,
    )
    .unwrap();
}
