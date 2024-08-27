use std::{ops::Sub, time::Duration};

use crate::common::{create_start_request_mock, create_system};
use actor::{ActorPath, ActorRef};
use identity::{
    identifier::{derive::digest::DigestDerivator, DigestIdentifier},
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{init_state, Event, Governance, Node, NodeMessage, Signature, Signed, Subject, SubjectCommand, SubjectResponse, SubjectsTypes, Validation, ValidationCommand, ValidationInfo};

#[tokio::test]
async fn test_local_validation() {
    let system = create_system().await;
    // Node
    let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
    let node = Node::new(&node_keys).unwrap();
    let node_actor = system.create_root_actor("node", node).await.unwrap();
    // Governance
    let request = create_start_request_mock(node_keys.clone(), node_keys.key_identifier().clone());
    let gov_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
    let event = Event::from_create_request(
        &gov_keys,
        &request,
        0,
        &init_state(&node_keys.key_identifier().to_string()),
        DigestDerivator::Blake3_256,
    )
    .unwrap();
    let signature =
        Signature::new(&event, &node_keys, DigestDerivator::Blake3_256)
            .unwrap();
    let signed_event = Signed {
        content: event.clone(),
        signature,
    };
    let subject = Subject::from_event(
        gov_keys.clone(),
        &signed_event,
    )
    .unwrap();
    node_actor.tell(NodeMessage::RegisterSubject(SubjectsTypes::OwnerGovernance(subject.subject_id.to_string()))).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let subject_actor = system.get_or_create_actor(&format!("node/{}", subject.subject_id), || subject.clone()).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let validation_actor: ActorRef<Validation> = system.get_actor(&ActorPath::from(format!("/user/node/{}/validation", subject.subject_id))).await.unwrap();
    let subject_state = subject_actor.ask(SubjectCommand::GetSubjectState).await.unwrap();
    let subject_state = if let SubjectResponse::SubjectState(state) = subject_state {
        state
    } else {
        panic!("Invalid response");
    };
    let request = create_start_request_mock(gov_keys.clone(), subject_state.subject_key.clone());
    let event = Event::from_create_request(
        &gov_keys,
        &request,
        0,
        &init_state(&node_keys.key_identifier().to_string()),
        DigestDerivator::Blake3_256,
    ).unwrap();
    let subject_signature = Signature::new(&event, &gov_keys, DigestDerivator::Blake3_256).unwrap();
    let subject_event = Signed {
        content: event,
        signature: subject_signature,
    };
    let info = ValidationInfo {
        subject : subject_state.clone(),
        event: subject_event,
        gov_version: 0,
    };
    validation_actor.tell(ValidationCommand::Create { request_id: DigestIdentifier::default(), info: info.clone() }).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    
}
