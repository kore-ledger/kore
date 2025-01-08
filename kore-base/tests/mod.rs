
// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{str::FromStr, time::Duration};

use identity::identifier::{DigestIdentifier, KeyIdentifier};
use kore_base::{
    approval::approver::ApprovalStateRes,
    model::{
        request::{CreateRequest, EventRequest, FactRequest},
        Namespace, ValueWrapper,
    },
};
use node_builder::create_nodes_and_connections;
use serde_json::json;

mod node_builder;

#[tokio::test]
#[tracing_test::traced_test]
async fn test_governance_copy() {
    // Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        false,
        46000,
    )
    .await;
    let node1 = &nodes[0];
    let node2 = &nodes[1];

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
                KeyIdentifier::from_str(&node1.controller_id()).unwrap(),
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
                        "id": node2.controller_id(),
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
                                "QUANTITY": 2
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

    tokio::time::sleep(Duration::from_secs(3)).await;

    let event = node2
        .get_first_or_end_events(subject_id.clone(), 5, false, Some(true))
        .await
        .unwrap();
    // verify if have 5 events
    if let Some(events_array) = event.get("events").and_then(|v| v.as_array()) {
        assert_eq!(events_array.len(), 5);
    } else {
        panic!("Expected 'events' to be an array but got {:?}", event);
    }
}

#[tokio::test]
#[tracing_test::traced_test]
async fn test_basic_use_case_1b_1e_1a() {
    //  Ephemeral -> Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![vec![0]],
        true,
        45000,
    )
    .await;
    let intermediary = &nodes[0];
    let owner_governance = &nodes[1];
    let emit_events = &nodes[2];

    let request = EventRequest::Create(CreateRequest {
        governance_id: DigestIdentifier::default(),
        schema_id: "governance".to_owned(),
        namespace: Namespace::new(),
    });

    // Create a governance in addresable node
    let data = owner_governance.own_request(request).await.unwrap();
    // Authorization the governance in bootstrap  and ephemeral node
    let governance_id = DigestIdentifier::from_str(&data.subject_id).unwrap();
    let response = intermediary
        .auth_subject(
            governance_id.clone(),
            kore_base::auth::AuthWitness::One(
                KeyIdentifier::from_str(&owner_governance.controller_id())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(response, "Ok");
    let response = emit_events
        .auth_subject(
            governance_id.clone(),
            kore_base::auth::AuthWitness::One(
                KeyIdentifier::from_str(&owner_governance.controller_id())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();

    assert_eq!(response, "Ok");

    tokio::time::sleep(Duration::from_secs(3)).await;

    // add node bootstrap and ephemeral to governance
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": emit_events.controller_id(),
                        "name": "KoreNode2"
                    }
                },
                {
                    "op": "add",
                    "path": "/members/2",
                    "value": {
                        "id": intermediary.controller_id(),
                        "name": "KoreNode3"
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/6",
                    "value": {
                        "namespace": "",
                        "role": {
                            "CREATOR": {
                                "QUANTITY": 2
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

    println!("Governance ID: {:?}", governance_id);

    let request = EventRequest::Fact(FactRequest {
        subject_id: governance_id.clone(),
        payload: ValueWrapper(json),
    });
    tokio::time::sleep(Duration::from_secs(5)).await;
    // send new governance version to addressable node
    let data = owner_governance.own_request(request).await.unwrap();
    tokio::time::sleep(Duration::from_secs(12)).await;
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let response = owner_governance
            .request_state(
                DigestIdentifier::from_str(&data.request_id).unwrap(),
            )
            .await
            .unwrap();
        println!("{}", response);
        if response == "Finish" {
            break;
        }
    }

    // verify the governance copy in bootstrap and ephemeral node
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;

        let bootstrap_response = intermediary.all_govs(None).await.unwrap();
        let ephemeral_response = emit_events.all_govs(None).await.unwrap();
        let addressable_response =
            owner_governance.all_govs(None).await.unwrap();

        println!("Bootstrap: {:?}", bootstrap_response);
        println!("Ephemeral: {:?}", ephemeral_response);
        println!("Addressable: {:?}", addressable_response);

        // Verificar que todos tengan al menos un elemento
        if !bootstrap_response.is_empty()
            && !ephemeral_response.is_empty()
            && !addressable_response.is_empty()
        {
            break;
        }

        intermediary
            .update_subject(governance_id.clone())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(6)).await;
        emit_events
            .update_subject(governance_id.clone())
            .await
            .unwrap();
    }
}

// verificar copias de la gobernanza entre nodos independientemente del tipo ✅
// verificar copias de los eventos de un sujeto independientemente del tipo  ✅
// si no esta autorizado verificar que no recibe la copia
// denegar cambios de gobernanza
// 