use std::{fs, str::FromStr, time::Duration};

use identity::{
    identifier::{
        derive::{digest::DigestDerivator, KeyDerivator},
        DigestIdentifier, KeyIdentifier,
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{
    approval::approver::ApprovalStateRes,
    config::{Config, ExternalDbConfig, KoreDbConfig},
    model::{
        request::{CreateRequest, EventRequest, FactRequest},
        Namespace, ValueWrapper,
    },
    Api,
};
use network::{Config as NetworkConfig, RoutingNode};
use prometheus_client::registry::Registry;
use serde_json::json;
use serial_test::serial;
use tokio_util::sync::CancellationToken;

pub fn create_temp_dir() -> String {
    let path = temp_dir();

    if fs::metadata(&path).is_err() {
        fs::create_dir_all(&path).unwrap();
    }
    path
}

fn temp_dir() -> String {
    let dir = tempfile::tempdir().expect("Can not create temporal directory.");
    dir.path().to_str().unwrap().to_owned()
}

async fn create_node1() -> Api {
    let keys = KeyPair::Ed25519(Ed25519KeyPair::new());

    let dir = tempfile::tempdir().expect("Can not create temporal directory.");
    let path = dir.path().to_str().unwrap();

    let newtork_config = NetworkConfig::new(
        network::NodeType::Bootstrap,
        vec!["/ip4/127.0.0.1/tcp/4500".to_owned()],
        vec![],
        vec![],
        false,
    );

    let config = Config {
        key_derivator: KeyDerivator::Ed25519,
        digest_derivator: DigestDerivator::Blake3_256,
        kore_db: KoreDbConfig::build(path),
        external_db: ExternalDbConfig::build(&format!(
            "{}/database.db",
            create_temp_dir()
        )),
        network: newtork_config,
        contracts_dir: create_temp_dir(),
        always_accept: false,
        garbage_collector: Duration::from_secs(500),
    };

    let mut registry = Registry::default();
    let token = CancellationToken::new();
    Api::new(keys, config, &mut registry, "kore", &token)
        .await
        .unwrap()
}

async fn create_node2(peer_id: &str) -> Api {
    let keys = KeyPair::Ed25519(Ed25519KeyPair::new());

    let dir = tempfile::tempdir().expect("Can not create temporal directory.");
    let path = dir.path().to_str().unwrap();

    let newtork_config = NetworkConfig::new(
        network::NodeType::Addressable,
        vec!["/ip4/127.0.0.1/tcp/4501".to_owned()],
        vec![],
        vec![RoutingNode {
            peer_id: peer_id.to_owned(),
            address: vec!["/ip4/127.0.0.1/tcp/4500".to_owned()],
        }],
        false,
    );

    let config = Config {
        key_derivator: KeyDerivator::Ed25519,
        digest_derivator: DigestDerivator::Blake3_256,
        kore_db: KoreDbConfig::build(path),
        external_db: ExternalDbConfig::build(&format!(
            "{}/database.db",
            create_temp_dir()
        )),
        network: newtork_config,
        contracts_dir: create_temp_dir(),
        always_accept: false,
        garbage_collector: Duration::from_secs(500),
    };

    let mut registry = Registry::default();
    let token = CancellationToken::new();
    Api::new(keys, config, &mut registry, "kore", &token)
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn test_governance_copy() {
    let node1 = create_node1().await;
    let peer_node1 = node1.peer_id();
    let controller_node1 = node1.controller_id();

    let node2 = create_node2(&peer_node1).await;
    let _peer_node2 = node2.peer_id();
    let controller_node2 = node2.controller_id();

    let request = EventRequest::Create(CreateRequest {
        governance_id: DigestIdentifier::default(),
        schema_id: "governance".to_owned(),
        namespace: Namespace::new(),
    });

    let data = node1.own_request(request).await.unwrap();
    let governance_id = DigestIdentifier::from_str(&data.subject_id).unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;
    let response = node1
        .request_state(DigestIdentifier::from_str(&data.request_id).unwrap())
        .await
        .unwrap();
    println!("{}", response);

    let response = node2
        .auth_subject(
            governance_id.clone(),
            kore_base::auth::AuthWitness::One(
                KeyIdentifier::from_str(&controller_node1).unwrap(),
            ),
        )
        .await
        .unwrap();
    println!("{}", response);

    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": controller_node2,
                        "name": "KoreNode2"
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/6",
                    "value": {
                        "namespace": "",
                        "role": {
                            "CREATOR": {
                                "quantity": 2
                            }
                        },
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode2"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/7",
                    "value": {
                        "namespace": "",
                        "role": "ISSUER",
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode2"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/policies/1",
                    "value": {
                        "id": "Example",
                        "approve": {
                            "quorum": {
                                "FIXED": 1
                            }
                        },
                        "evaluate": {
                            "quorum": "MAJORITY"
                        },
                        "validate": {
                            "quorum": "MAJORITY"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/schemas/0",
                    "value": {
                        "contract": {
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS50aHJlZSA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZEFsbCB7IG9uZSwgdHdvLCB0aHJlZSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBvbmU7CiAgICAgICAgc3RhdGUudHdvID0gdHdvOwogICAgICAgIHN0YXRlLnRocmVlID0gdGhyZWU7CiAgICAgIH0KICB9CiAgY29udHJhY3RfcmVzdWx0LnN1Y2Nlc3MgPSB0cnVlOwp9Cgo="
                        },
                        "id": "Example",
                        "initial_value": {
                            "one": 0,
                            "two": 0,
                            "three": 0
                        }
                    }
                }
    ]}});

    let request = EventRequest::Fact(FactRequest {
        subject_id: governance_id.clone(),
        payload: ValueWrapper(json),
    });

    let data = node1.own_request(request).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let response = node1
        .request_state(DigestIdentifier::from_str(&data.request_id).unwrap())
        .await
        .unwrap();
    println!("{}", response);

    node1
        .approve(governance_id.clone(), ApprovalStateRes::RespondedAccepted)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let response = node1
        .request_state(DigestIdentifier::from_str(&data.request_id).unwrap())
        .await
        .unwrap();
    println!("{}", response);

    tokio::time::sleep(Duration::from_secs(3)).await;
    let _response = node2.get_subject(governance_id.clone()).await.unwrap();
    //println!("{:?}", response);

    let request = EventRequest::Create(CreateRequest {
        governance_id: governance_id.clone(),
        schema_id: "Example".to_owned(),
        namespace: Namespace::new(),
    });

    let data = node2.own_request(request).await.unwrap();
    let subject_id = DigestIdentifier::from_str(&data.subject_id).unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;
    let _response = node2.get_subject(subject_id.clone()).await.unwrap();

    let _response = node1.get_subject(subject_id.clone()).await.unwrap();

    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });

    let request = EventRequest::Fact(FactRequest {
        subject_id: subject_id.clone(),
        payload: ValueWrapper(json),
    });

    let _data = node2.own_request(request).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let response = node2
        .get_events(subject_id.clone(), Some(5), Some(1))
        .await
        .unwrap();
    println!("");
    println!("{:?}", response);
    println!("");

    let response = node1.get_subject(subject_id.clone()).await.unwrap();
    println!("{:?}", response);

    for i in 0..250 {
        let json = json!({
            "ModTwo": {
                "data": i + 1,
            }
        });

        let request = EventRequest::Fact(FactRequest {
            subject_id: subject_id.clone(),
            payload: ValueWrapper(json),
        });

        let _data = node2.own_request(request).await.unwrap();
    }
}
