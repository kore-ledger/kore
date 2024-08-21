use std::{collections::HashSet, ops::Sub, time::Duration};

use crate::common::{
    create_start_request_mock, create_system, fact_request_mock,
};
use actor::{ActorPath, ActorRef};
use identity::{
    identifier::{derive::digest::DigestDerivator, DigestIdentifier},
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{
    init_state, Event, Governance, HashId, Node, NodeMessage, NodeResponse,
    Signature, Signed, Subject, SubjectCommand, SubjectResponse, SubjectsTypes,
    Validation, ValidationCommand, ValidationInfo, ValueWrapper,
};
use tracing_subscriber::EnvFilter;
use tracing_test::traced_test;


async fn initialize_logger() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}

#[tokio::test]
async fn test_local_validation() {
    // Initialize logger
    initialize_logger().await;

    // Create system
    let system = create_system().await;

    // Create Node
    let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
    let node = Node::new(&node_keys, DigestDerivator::Blake3_256).unwrap();
    let node_actor = system
        .create_root_actor("node", node.clone())
        .await
        .unwrap();

    // Create Governance
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
    let signature =
        Signature::new(&event, &node_keys, DigestDerivator::Blake3_256)
            .unwrap();
    let signed_event = Signed {
        content: event.clone(),
        signature,
    };
    let subject = Subject::from_event(
        gov_keys.clone(),
        DigestDerivator::Blake3_256,
        &signed_event,
    )
    .unwrap();

    // Register subject
    node_actor
        .tell(NodeMessage::RegisterSubject(
            SubjectsTypes::OwnerGovernance(subject.subject_id.to_string()),
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create Subject actor
    let subject_actor = system
        .get_or_create_actor(&format!("node/{}", subject.subject_id), || {
            subject.clone()
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Verify is validator actor is created
    let validation_actor: ActorRef<Validation> = system
        .get_actor(&ActorPath::from(format!(
            "/user/node/{}/validation",
            subject.subject_id
        )))
        .await
        .unwrap();

    // Obtain subject state
    let subject_state = subject_actor
        .ask(SubjectCommand::GetSubjectState)
        .await
        .unwrap();
    let subject_state =
        if let SubjectResponse::SubjectState(state) = subject_state {
            state
        } else {
            panic!("Invalid response");
        };

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
    let subject_signature =
        Signature::new(&event, &gov_keys, DigestDerivator::Blake3_256).unwrap();
    let subject_event = Signed {
        content: event,
        signature: subject_signature.clone(),
    };
    let info = ValidationInfo {
        subject: subject_state.clone(),
        event: subject_event.clone(),
        gov_version: 0,
    };
    // Send message to validation actor, to create a validator actor
    validation_actor
        .tell(ValidationCommand::Create {
            request_id: DigestIdentifier::default(),
            info: info.clone(),
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Stop node and subject actors
    node_actor.stop().await;
    subject_actor.stop().await;

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Up node actor
    let node_actor = system
        .create_root_actor("node", node.clone())
        .await
        .unwrap();
    let response = node_actor
        .ask(NodeMessage::AmIGovernanceOwner(subject.subject_id.clone()))
        .await
        .unwrap();

    // Verify persistence of node actor
    if let NodeResponse::AmIOwner(is_owner) = response {
        assert!(is_owner);
    } else {
        panic!("Invalid response");
    }
    // Up subject actor
    let _ = system
        .get_or_create_actor(&format!("/node/{}", subject.subject_id), || {
            subject.clone()
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify is validator actor is created
    let validation_actor: ActorRef<Validation> = system
        .get_actor(&ActorPath::from(format!(
            "/user/node/{}/validation",
            subject.subject_id
        )))
        .await
        .unwrap();

    // Create Fact Request
    let request =
        fact_request_mock(gov_keys.clone(), subject.subject_id.clone());

    // Calculate event hash of first event
    let event_hash = info.event.hash_id(DigestDerivator::Blake3_256).unwrap();

    let fact_event = Event {
        subject_id: subject.subject_id.clone(),
        event_request: request.clone(),
        sn: 1,
        gov_version: 0,
        patch: ValueWrapper::default(),
        state_hash: event_hash.clone(),
        eval_success: true,
        appr_required: false,
        approved: true,
        hash_prev_event: event_hash,
        evaluators: HashSet::default(),
        approvers: HashSet::new(),
    };

    let subject_event = Signed {
        content: fact_event,
        signature: subject_signature,
    };

    let second_info = ValidationInfo {
        subject: subject_state.clone(),
        event: subject_event,
        gov_version: 0,
    };
    // Send message to validation actor, to create a validator actor with second event(verify persistence of validator actor)
    validation_actor
        .tell(ValidationCommand::Create {
            request_id: DigestIdentifier::default(),
            info: second_info.clone(),
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
}
