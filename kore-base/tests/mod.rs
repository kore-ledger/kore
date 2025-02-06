// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{str::FromStr, time::Duration};

use identity::{
    identifier::KeyIdentifier,
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{approval::approver::ApprovalStateRes, auth::AuthWitness};
use node_builder::{
    create_and_authorize_governance, create_nodes_and_connections,
    create_subject, emit_confirm, emit_fact, emit_transfer,
    wait_for_governance_sync,
};
use serde_json::json;
use serial_test::serial;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

mod node_builder;

#[tokio::test]
#[serial]
//  Verificar que se puede crear una gobernanza, sujeto y emitir un evento además de recibir la copia
async fn test_governance_and_subject_copy_with_approve() {
    let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::TRACE)
    .finish();

// Establece el subscriber global
tracing::subscriber::set_global_default(subscriber)
    .expect("setting default subscriber failed"); 
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

    let governance_id =
        create_and_authorize_governance(node1, &[node2], "").await;

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

    emit_fact(node1, governance_id.clone(), json, None)
        .await
        .unwrap();

    node1
        .approve(governance_id.clone(), ApprovalStateRes::RespondedAccepted)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    // verify copy in all node
    wait_for_governance_sync(governance_id.clone(), &[node1, node2], 3, 1, 3)
        .await
        .unwrap();

    let subject_id =
        create_subject(node2, governance_id.clone(), "Example", "")
            .await
            .unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });

    emit_fact(node2, subject_id.clone(), json, None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    for i in 0..10 {
        let json = json!({
            "ModTwo": {
                "data": i + 1,
            }
        });

        emit_fact(node2, subject_id.clone(), json, None)
            .await
            .unwrap();
    }

    let events = node2
        .get_first_or_end_events(
            subject_id.clone(),
            Some(5),
            Some(false),
            Some(true),
        )
        .await
        .unwrap();

    assert_eq!(events.len(), 5);
}

#[tokio::test]
#[serial]
#[tracing_test::traced_test]
// Caso de uso básico 1 bootstrap (intermediario), 1 ephemeral(issuer de subject),
// 1 addressable(owner de la gobernanza)
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

    let governance_id = create_and_authorize_governance(
        owner_governance,
        &[intermediary, emit_events],
        "",
    )
    .await;

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

    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;
    // wait for sync
    wait_for_governance_sync(
        governance_id.clone(),
        &[intermediary, emit_events],
        5,
        1,
        5,
    )
    .await
    .unwrap();
}

#[tokio::test]
#[serial]
#[tracing_test::traced_test]
// Testear limitaciones en la creación de sujetos INFINITY - QUANTITY
async fn test_limits_in_subjects() {
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

    let governance_id = create_and_authorize_governance(
        owner_governance,
        &[intermediary, emit_events],
        "",
    )
    .await;

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
                                "QUANTITY": 1
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

    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;
    // wait for sync
    wait_for_governance_sync(
        governance_id.clone(),
        &[intermediary, emit_events],
        5,
        1,
        1,
    )
    .await
    .unwrap();

    let subject_id =
        create_subject(emit_events, governance_id.clone(), "Example", "")
            .await
            .unwrap();
    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });
    emit_fact(emit_events, subject_id.clone(), json, None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;

    // create other subject and error
    let subject_id =
        create_subject(emit_events, governance_id.clone(), "Example", "").await;
    assert!(subject_id.is_err());

    // modify the governance to allow more subjects
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "replace",
                    "path": "/roles/6/role/CREATOR/QUANTITY",
                    "value": 5
                }
            ]
    }});
    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    // wait for sync
    wait_for_governance_sync(
        governance_id.clone(),
        &[intermediary, emit_events],
        5,
        2,
        1,
    )
    .await
    .unwrap();

    // create other subject
    let _ = create_subject(emit_events, governance_id.clone(), "Example", "")
        .await
        .unwrap();

    // now we have two subjects, modify the governance to allow only one
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "replace",
                    "path": "/roles/6/role/CREATOR/QUANTITY",
                    "value": 1
                }
            ]
    }});
    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();
    wait_for_governance_sync(
        governance_id.clone(),
        &[intermediary, emit_events],
        5,
        3,
        1,
    )
    .await
    .unwrap();
    let subject =
        create_subject(emit_events, governance_id.clone(), "Example", "").await;
    assert!(subject.is_err());
}

#[tokio::test]
#[serial]
#[ignore = "Verify Validator Namespace"]
#[tracing_test::traced_test]
// Testear los esppacios de nombre
async fn test_namespace_in_role() {
    // ValidationRes, Can not safe ledger or event: Actor /user/node/J5Vdfg0zP5LXj0WhnEJmYQTuL0Plu8vYB0GdB_j8cQl8/ledger_event not found
    // Se esta borrando el sujeto
    // Porque no encontro validadores?¿?¿? Cuando esta el owner, y el KoreNode2
    // get_quorum_and_signers
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        46000,
    )
    .await;
    let intermediary = &nodes[0];
    let owner_governance = &nodes[1];
    let emit_events = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        &[intermediary, emit_events],
        "",
    )
    .await;

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Tiene sentido que el las politicas del schema se ponga el rol de aprovador???
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
                        "namespace": "test",
                        "role": {
                            "CREATOR": {
                                "QUANTITY": 1
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
                        "namespace": "test1",
                        "role": {
                            "CREATOR": {
                                "QUANTITY": 1
                            }
                        },
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode3"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/8",
                    "value": {
                        "namespace": "test1",
                        "role": "EVALUATOR",
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode3"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/9",
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
                                "FIXED": 2
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
    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    wait_for_governance_sync(
        governance_id.clone(),
        &[intermediary, emit_events, owner_governance],
        5,
        1,
        5,
    )
    .await
    .unwrap();
    // create subject
    let subject_id =
        create_subject(emit_events, governance_id.clone(), "Example", "").await;
    assert!(subject_id.is_err());
    // create subject in namespace
    let _ =
        create_subject(emit_events, governance_id.clone(), "Example", "test")
            .await
            .unwrap();
    // create other subject in namespace
    let subject_id =
        create_subject(intermediary, governance_id.clone(), "Example", "test1")
            .await
            .unwrap();
    println!("Subject ID: {:?}", subject_id);
    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });
    emit_fact(intermediary, subject_id.clone(), json, None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    let signature = intermediary.all_govs(None).await.unwrap();
    assert_eq!(governance_id.to_string(), signature[0].governance_id);
    // get subjects
    let subjects = intermediary
        .all_subjs(governance_id, None, None)
        .await
        .unwrap();
    println!("Signature: {:?}", subjects);
}

#[tokio::test]
#[serial]
#[tracing_test::traced_test]
// copia de varias modificaciones en la gobernanza
async fn test_copy_many_events() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45000,
    )
    .await;
    let intermediary = &nodes[0];
    let owner_governance = &nodes[1];

    let governance_id =
        create_and_authorize_governance(owner_governance, &[intermediary], "")
            .await;

    // emit 100 events in governance
    for i in 1..3 {
        let json = json!({ "Patch": {
            "data": [ {
                "op": "add",
                "path": format!("/members/{}", i),
                "value": {
                    "id": KeyPair::Ed25519(Ed25519KeyPair::new()).key_identifier().to_string(),
                    "name": format!("K{}", i)
                }
            }]
        }});
        emit_fact(owner_governance, governance_id.clone(), json, Some(true))
            .await
            .unwrap();
    }
    // add new member in governance
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": intermediary.controller_id(),
                        "name": "KoreNode2"
                    }
                }
            ]
    }});
    tokio::time::sleep(Duration::from_secs(5)).await;
    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    // state of governance
    let state = owner_governance
        .get_subject(governance_id.clone())
        .await
        .unwrap();
    println!("State: {:?}", state);
    wait_for_governance_sync(
        governance_id.clone(),
        &[intermediary, owner_governance],
        10,
        3,
        2,
    )
    .await
    .unwrap();
}

#[tokio::test]
#[serial]
#[tracing_test::traced_test]
// Modificar el estado inicial de la gobernanza
async fn test_modify_init_state_governance() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45000,
    )
    .await;
    let intermediary = &nodes[0];
    let owner_governance = &nodes[1];

    let governance_id =
        create_and_authorize_governance(owner_governance, &[intermediary], "")
            .await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    let response = owner_governance
        .get_subject(governance_id.clone())
        .await
        .unwrap();
    println!("Response: {:?}", response);
    // MODIFY INITIAL STATE(controller id)
    let new_key = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();

    let json_patch = json!({"Patch": {
        "data": [
            {
                "op": "replace",
                "path": "/members/0/id",
                "value": new_key
            }
        ]
    }});

    // Luego, llamas a tu emit_fact
    emit_fact(owner_governance, governance_id.clone(), json_patch, None)
        .await
        .unwrap();

    // modify name owner (not possible to modify)
    let json_patch = json!({"Patch": {
        "data": [
            {
                "op": "replace",
                "path": "/members/0/name",
                "value": "KoreNode2"
            }
        ]
    }});
    emit_fact(owner_governance, governance_id.clone(), json_patch, None)
        .await
        .unwrap();
    let state = owner_governance
        .get_subject(governance_id.clone())
        .await
        .unwrap();
    assert!(state.sn == 1);
    assert!(state.properties["version"] == 1);
}

#[tokio::test]
#[serial]
// Modificar el estado inicial del sujeto
async fn test_modify_init_state_subject() {
    // 1 sujeto con 2 eventos, luego cambiar la gobernanza , luego
/*     let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    // Establece el subscriber global
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed"); */
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45000,
    )
    .await;
    let owner_governance = &nodes[0];
    let intermediary = &nodes[1];

    let governance_id =
        create_and_authorize_governance(owner_governance, &[intermediary], "")
            .await;

    // add schema to governance
    let json = json!({"Patch": {
        "data": [
            {
                "op": "add",
                "path": "/roles/6",
                "value": {
                    "namespace": "",
                    "role": {
                        "CREATOR": {
                            "QUANTITY": 5
                        }
                    },
                    "schema": {
                        "ID": "Example"
                    },
                    "who": {
                        "NAME": "Owner"
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
                        "NAME": "Owner"
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
                        "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBzdGF0ZS5vbmUgKyBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IHN0YXRlLnR3byArIGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS50aHJlZSA9IHN0YXRlLnRocmVlICsgZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCg=="
                    },
                    "id": "Example",
                    "initial_value": {
                        "one": 1,
                        "two": 1,
                        "three": 1
                    }
                }
            }
        ]
    }});
    // emit event to governance
    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();
    // create subject
    let subject_id =
        create_subject(owner_governance, governance_id.clone(), "Example", "")
            .await
            .unwrap();
    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });
    emit_fact(owner_governance, subject_id.clone(), json, None)
        .await
        .unwrap();
    // obtain state of subject
    let state = owner_governance
        .get_subject(subject_id.clone())
        .await
        .unwrap();
    println!("State: {:?}", state);
    // modify initial state of subject
    let json = json!({"Patch": {
        "data": [
            {
                "op": "add",
                "path": "/members/1",
                "value": {
                    "id": intermediary.controller_id(),
                    "name": "KoreNode2"
                }
            },
            {
                "op": "add",
                "path": "/roles/1",
                "value": {
                    "namespace": "",
                    "role": "WITNESS",
                    "schema": {
                        "ID": "Example"
                    },
                    "who": {
                        "NAME": "KoreNode2"
                    }
                }
            },
            {
                "op": "replace",
                "path": "/schemas/0/initial_value/one",
                "value": 33
            }
        ]
    }});
    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();
    wait_for_governance_sync(
        governance_id.clone(),
        &[intermediary, owner_governance],
        5,
        2,
        5,
    )
    .await
    .unwrap();
    // obtain state of governance
    let state = owner_governance
        .get_subject(governance_id.clone())
        .await
        .unwrap();
    println!("State governance: {:?}", state);
    // autorizamos al testigo para que reciba copia del sujeto y hacemos un update
    intermediary
        .auth_subject(
            subject_id.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&owner_governance.controller_id())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
    loop {
        // un testigo tiene q recuperar una copia del evento
        intermediary
            .update_subject(subject_id.clone())
            .await
            .unwrap();
        // obtain state of subject
        let state = intermediary.get_subject(subject_id.clone()).await;
        if state.is_ok() {
            let governance_state = intermediary
                .get_subject(governance_id.clone())
                .await
                .unwrap();
            println!("State governance: {:?}", governance_state);

            println!("State intermediary: {:?}", state);
            break;
        }
    }

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });
    emit_fact(owner_governance, subject_id.clone(), json, None)
        .await
        .unwrap();
    // obtain state of subject
    let state = owner_governance
        .get_subject(subject_id.clone())
        .await
        .unwrap();
    println!("State: {:?}", state);
    // da 201 claro estas de cierta manera manipulando el estado??
    // hacer update
    loop {
        intermediary
            .update_subject(subject_id.clone())
            .await
            .unwrap();
        // obtain state of subject
        let state = intermediary.get_subject(subject_id.clone()).await;
        if state.is_ok() {
            println!("State intermediary: {:?}", state);
            break;
        }
    }
}

#[tokio::test]
#[serial]
#[ignore = "Need to change members in governance"]
#[tracing_test::traced_test]
// Testear la transferencia de gobernanza
async fn test_transfer_governance_event() {
    // error: Some("Invalid request signer")
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45000,
    )
    .await;
    let future_owner = &nodes[0];
    let owner_governance = &nodes[1];

    println!("Owner: {:?}", owner_governance.controller_id());
    println!("Future Owner: {:?}", future_owner.controller_id());

    let governance_id =
        create_and_authorize_governance(owner_governance, &[future_owner], "")
            .await;
    // add member to governance
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": future_owner.controller_id(),
                        "name": "KoreNode1"
                    }
                }
            ]
    }});
    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();

    wait_for_governance_sync(
        governance_id.clone(),
        &[future_owner, owner_governance],
        5,
        1,
        5,
    )
    .await
    .unwrap();

    emit_transfer(
        owner_governance,
        governance_id.clone(),
        KeyIdentifier::from_str(&future_owner.controller_id()).unwrap(),
        None,
    )
    .await
    .unwrap();
    owner_governance
        .auth_subject(
            governance_id.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&future_owner.controller_id()).unwrap(),
            ),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await;
    // Confirm transfer event
    emit_confirm(future_owner, governance_id.clone(), None).await;

    // add new fake member to governance
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/2",
                    "value": {
                        "id": KeyPair::Ed25519(Ed25519KeyPair::new()).key_identifier().to_string(),
                        "name": "KoreNode2"
                    }
                }
            ]
    }});
    emit_fact(future_owner, governance_id.clone(), json, None)
        .await
        .unwrap();

    let state = future_owner
        .get_subject(governance_id.clone())
        .await
        .unwrap();

    println!("State: {:?}", state);
}

#[tokio::test]
#[serial]
// Testear la transferencia de sujeto
async fn test_transfer_subject_event() {
        let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    // Establece el subscriber global
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed"); 
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45000,
    )
    .await;
    let future_owner = &nodes[0];
    let owner_governance = &nodes[1];

    println!("Owner: {:?}", owner_governance.controller_id());
    println!("Future Owner: {:?}", future_owner.controller_id());

    let governance_id =
        create_and_authorize_governance(owner_governance, &[future_owner], "")
            .await;

    // add member to governance
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": future_owner.controller_id(),
                        "name": "KoreNode1"
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/1",
                    "value": {
                        "namespace": "",
                        "role": {
                            "CREATOR": "INFINITY"
                        },
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "Owner"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/2",
                    "value": {
                        "namespace": "",
                        "role": "ISSUER",
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "Owner"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/3",
                    "value": {
                        "namespace": "",
                        "role": {
                            "CREATOR": "INFINITY"
                        },
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode1"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/policies/1",
                    "value": {
                        "id": "Example",
                        "approve": {
                            "quorum": "MAJORITY"
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
            ]
    }});
    emit_fact(owner_governance, governance_id.clone(), json, None)
        .await
        .unwrap();

    // create subject
    let subject_id =
        create_subject(owner_governance, governance_id.clone(), "Example", "")
            .await
            .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });
    emit_fact(owner_governance, subject_id.clone(), json.clone(), None)
        .await
        .unwrap();

    let state = owner_governance
        .get_subject(subject_id.clone())
        .await
        .unwrap();
    println!("State: {:?}", state);

    // autorizar para recibir copias del nuevo sujeto
    future_owner
        .auth_subject(
            subject_id.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&owner_governance.controller_id())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();

    // transfer subject
    emit_transfer(
        owner_governance,
        subject_id.clone(),
        KeyIdentifier::from_str(&future_owner.controller_id()).unwrap(),
        None,
    )
    .await
    .unwrap();

    // pedir al antiguo owner la copia del sujeto
    loop {
        future_owner
            .update_subject(subject_id.clone())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        let state = future_owner.get_subject(subject_id.clone()).await;
        if state.is_ok() {
            println!("State: {:?}", state);
            break;
        }
    }

    tokio::time::sleep(Duration::from_secs(5)).await;
    println!("Emitted transfer");

    // confirm transfer
    // analizar que pasa si no tienes el rol de creador
    emit_confirm(future_owner, subject_id.clone(), None).await;

    tokio::time::sleep(Duration::from_secs(5)).await;
    println!("Emitted transfer");

    // emit event to subject in the last owner
    let result =
        emit_fact(owner_governance, subject_id.clone(), json.clone(), None)
            .await;
    assert!(result.is_err());

    let result =
        emit_fact(future_owner, subject_id.clone(), json.clone(), None).await;
    assert!(result.is_ok());
    // state of subject
    let state = future_owner.get_subject(subject_id.clone()).await.unwrap();
    println!("State: {:?}", state);
    assert!(state.sn == 3);
}

// verificar copias de la gobernanza entre nodos independientemente del tipo ✅
// verificar copias de los eventos de un sujeto independientemente del tipo  ✅
// si no esta autorizado verificar que no recibe la copia ✅
// revisar la limitación en la creación de sujetos y roles . ✅
// creación de sujetos bajo espacios de nombres ✅
// not members solo funciona para ISSUER❌
// MODIFICAR OWNER DE LA GOBERNANZA ✅
// PROBAR LA TRANSFERENCIA ❌
// CREAR GOVERNANZAS CON NAMESPACES ✅
// estar en la red y recibir copia y no ser mienbro
// probar la manual distribution ✅
// probar cosas entre un emit y un confirm