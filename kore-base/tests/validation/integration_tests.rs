use std::time::Duration;

use crate::common::{
    create_info_gov_event, create_info_gov_genesis_event, create_network, create_system, get_peer_id, initilize_use_case
};
use actor::{ActorPath, ActorRef};
use identity::
    identifier::DigestIdentifier
;
use kore_base::{
    NodeMessage, NodeResponse, SubjectCommand, SubjectResponse, SubjectsTypes,
    Validation, ValidationCommand, ValidationInfo, ValueWrapper,
};
use network::RoutingNode;
use tracing_subscriber::EnvFilter;

async fn initialize_logger() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}


// DigestDerivator::Blake3_256
#[tokio::test]
async fn test_local_validation() {
    // Initialize logger
    initialize_logger().await;

    // Create system
    let system = create_system().await;

    let (node, node_keys, subject, gov_keys) = initilize_use_case().await;

    // Create Node
    let node_actor = system
        .create_root_actor("node", node.clone())
        .await
        .unwrap();

    // Register subject in actor node
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
    let info = create_info_gov_genesis_event(
        node_keys.clone(),
        gov_keys.clone(),
        subject_state.clone(),
    );
    let info = ValidationInfo {
        subject: subject_state.clone(),
        event: info.clone(),
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
    let second_info = create_info_gov_event(
        gov_keys.clone(),
        subject_state.subject_id.clone(),
        info.clone(),
        ValueWrapper::default(),
    );
    let second_info = ValidationInfo {
        subject: subject_state.clone(),
        event: second_info.clone(),
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

#[tokio::test]
async fn test_network_validation() {
    initialize_logger().await;
    // Need to create 2 nodes
    let (node1, node_keys1, subject1, gov_keys1) = initilize_use_case().await;
    let (node2, node_keys2, _, _) = initilize_use_case().await;

    // Create systems
    let system1 = create_system().await;
    let system2 = create_system().await;

    create_network(
        system1.clone(),
        format!("/ip4/127.0.0.1/tcp/54422"),
        node_keys1.clone(),
        vec![],
    )
    .await;

    tokio::time::sleep(Duration::from_secs(2)).await;

    create_network(
        system2.clone(),
        format!("/ip4/127.0.0.1/tcp/54423"),
        node_keys2.clone(),
        vec![RoutingNode {
            peer_id: get_peer_id(node_keys1.clone()).to_string(),
            address: vec![format!("/ip4/127.0.0.1/tcp/54422")],
        }],
    )
    .await;

    // Create node actors
    let node_actor1 = system1
        .create_root_actor("node", node1.clone())
        .await
        .unwrap();

    let node_actor2 = system2
        .create_root_actor("node", node2.clone())
        .await
        .unwrap();

    // Register subject in actor node with owner
    node_actor1
        .tell(NodeMessage::RegisterSubject(
            SubjectsTypes::OwnerGovernance(subject1.subject_id.to_string()),
        ))
        .await
        .unwrap();
    // Register subject in actor node with know governance
    node_actor2
        .tell(NodeMessage::RegisterSubject(SubjectsTypes::KnowGovernance(
            subject1.subject_id.to_string(),
        )))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create Subjects actor
    let subject_actor1 = system1
        .get_or_create_actor(&format!("node/{}", subject1.subject_id), || {
            subject1.clone()
        })
        .await
        .unwrap();

    let subject_actor2 = system2
        .get_or_create_actor(&format!("node/{}", subject1.subject_id), || {
            subject1.clone()
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Obtain subject state
    let subject_state = subject_actor1
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
    let info = create_info_gov_genesis_event(
        node_keys1.clone(),
        gov_keys1.clone(),
        subject_state.clone(),
    );
    let info = ValidationInfo {
        subject: subject_state.clone(),
        event: info.clone(),
        gov_version: 0,
    };

    let validation_actor: ActorRef<Validation> = system1
        .get_actor(&ActorPath::from(format!(
            "/user/node/{}/validation",
            subject_state.subject_id.clone()
        )))
        .await
        .unwrap();
    
    // Create validatio request -> LocalValidation
    validation_actor
        .tell(ValidationCommand::Create {
            request_id: DigestIdentifier::default(),
            info: info.clone(),
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // UPDATE subject to add second node as a validator
    let update_subject = create_info_gov_event(
        gov_keys1,
        subject_state.subject_id.clone(),
        info,
        ValueWrapper(serde_json::json!([
            {
                "op": "add",
                "path": "/roles/1",
                "value": {
                    "namespace": "",
                    "role": "VALIDATOR",
                    "schema": {
                        "ID": "governance"
                    },
                    "who": {
                        "NAME": "Kore2"
                    }
                }
            },
            {
                "op": "add",
                "path": "/members/1",
                "value": {
                    "id": node_keys2.key_identifier(),
                    "name": "Kore2"
                }
            }
        ])),
    );

    subject_actor1
        .tell(SubjectCommand::UpdateSubject {
            event: update_subject.clone(),
        })
        .await
        .unwrap();

    subject_actor2
        .tell(SubjectCommand::UpdateSubject {
            event: update_subject.clone(),
        })
        .await
        .unwrap();

     tokio::time::sleep(Duration::from_secs(2)).await;
    // Stop subject actor of system2 to use prestar and create validator actor
    // TODO: al actualizar el sujeto se deben lanzar los los actores de validacion
    subject_actor2.stop().await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    let _ = system2
        .get_or_create_actor(&format!("node/{}", subject1.subject_id), || {
            subject1.clone()
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Obtain governance
    let gov = subject_actor1
        .ask(SubjectCommand::GetGovernance)
        .await
        .unwrap();
    let gov = if let SubjectResponse::Governance(gov) = gov {
        gov
    } else {
        panic!("Invalid response");
    };

    let validation_info = ValidationInfo {
        subject: subject_state.clone(),
        event: update_subject.clone(),
        gov_version: 1,
    };

    // Envio el evento a validar
    // TODO: Ya existe un actor de validator, por eso es probable que de error 
    validation_actor
        .tell(ValidationCommand::Create {
            request_id: DigestIdentifier::default(),
            info: validation_info,
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(10)).await; 
}
