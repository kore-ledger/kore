// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::str::FromStr;

use identity::{
    identifier::KeyIdentifier,
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{
    approval::approver::ApprovalStateRes,
    auth::AuthWitness,
    model::request::{ConfirmRequest, EventRequest},
};
use node_builder::{
    check_transfer, create_and_authorize_governance,
    create_nodes_and_connections, create_subject, emit_approve, emit_confirm,
    emit_fact, emit_reject, emit_transfer, get_subject,
};
use serde_json::json;
use test_log::test;

mod node_builder;

#[test(tokio::test)]
//  Verificar que se puede crear una gobernanza, sujeto y emitir un evento además de recibir la copia
async fn test_governance_and_subject_copy_with_approve() {
    // Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        false,
        45000,
    )
    .await;
    let node1 = &nodes[0];
    let node2 = &nodes[1];

    let governance_id =
        create_and_authorize_governance(node1, vec![node2], "").await;

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
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0="
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

    let request_id = emit_fact(node1, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_approve(
        node1,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id,
        true,
    )
    .await
    .unwrap();

    let subject_id =
        create_subject(node2, governance_id.clone(), "Example", "", true)
            .await
            .unwrap();

    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });

    emit_fact(node2, subject_id.clone(), json, true)
        .await
        .unwrap();

    for i in 0..9 {
        let json = json!({
            "ModTwo": {
                "data": i + 1,
            }
        });

        emit_fact(node2, subject_id.clone(), json, false)
            .await
            .unwrap();
    }

    let json = json!({
        "ModTwo": {
            "data": 9 + 1,
        }
    });

    emit_fact(node2, subject_id.clone(), json, true)
        .await
        .unwrap();

    let events = node2
        .get_first_or_end_events(
            subject_id.clone(),
            Some(11),
            Some(false),
            Some(true),
        )
        .await
        .unwrap();

    assert_eq!(events.len(), 11);

    let state = get_subject(node1, subject_id.clone()).await.unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, node2.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, node2.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 11);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 10
        })
    );

    let state = get_subject(node2, subject_id.clone()).await.unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, node2.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, node2.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 11);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 10
        })
    );
}

#[test(tokio::test)]
// Caso de uso básico 1 bootstrap (intermediario), 1 ephemeral(issuer de subject),
// 1 addressable(owner de la gobernanza)
async fn test_basic_use_case_1b_1e_1a() {
    //  Ephemeral -> Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![vec![0]],
        true,
        45010,
    )
    .await;
    let intermediary = &nodes[0];
    let owner_governance = &nodes[1];
    let emit_events = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![intermediary, emit_events],
        "",
    )
    .await;

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
                }
    ]}});

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_subject(owner_governance, governance_id.clone())
        .await
        .unwrap();

    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, "");
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner_governance.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    println!("{}", state.properties);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":owner_governance.controller_id(),"name":"Owner"},{"id":emit_events.controller_id(),"name":"KoreNode2"},{"id":intermediary.controller_id(),"name":"KoreNode3"}],"policies":[{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[],"version":1})
    );

    let state = get_subject(intermediary, governance_id.clone())
        .await
        .unwrap();

    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, "");
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner_governance.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":owner_governance.controller_id(),"name":"Owner"},{"id":emit_events.controller_id(),"name":"KoreNode2"},{"id":intermediary.controller_id(),"name":"KoreNode3"}],"policies":[{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[],"version":1})
    );

    let state = get_subject(emit_events, governance_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, "");
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner_governance.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":owner_governance.controller_id(),"name":"Owner"},{"id":emit_events.controller_id(),"name":"KoreNode2"},{"id":intermediary.controller_id(),"name":"KoreNode3"}],"policies":[{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[],"version":1})
    );
}

#[test(tokio::test)]
// Testear limitaciones en la creación de sujetos INFINITY - QUANTITY
async fn test_limits_in_subjects() {
    //  Ephemeral -> Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45020,
    )
    .await;

    let owner_governance = &nodes[0];
    let emit_events = &nodes[1];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![emit_events],
        "",
    )
    .await;

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
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0="
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

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let subject_id_1 =
        create_subject(emit_events, governance_id.clone(), "Example", "", true)
            .await
            .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });
    emit_fact(emit_events, subject_id_1.clone(), json, true)
        .await
        .unwrap();

    // create other subject and error
    let subject_id_error = create_subject(
        emit_events,
        governance_id.clone(),
        "Example",
        "",
        false,
    )
    .await;
    assert!(subject_id_error.is_err());

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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create other subject
    let subject_id_2 =
        create_subject(emit_events, governance_id.clone(), "Example", "", true)
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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let subject_id_error = create_subject(
        emit_events,
        governance_id.clone(),
        "Example",
        "",
        false,
    )
    .await;
    assert!(subject_id_error.is_err());

    let json = json!({
        "ModOne": {
            "data": 200,
        }
    });
    emit_fact(emit_events, subject_id_2.clone(), json, true)
        .await
        .unwrap();

    let state = get_subject(emit_events, subject_id_1.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_1.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    let state = get_subject(owner_governance, subject_id_1.clone())
        .await
        .unwrap();

    assert_eq!(state.subject_id, subject_id_1.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    let state = get_subject(emit_events, subject_id_2.clone())
        .await
        .unwrap();

    assert_eq!(state.subject_id, subject_id_2.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 2);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 200, "three": 0, "two": 0
        })
    );

    let state = get_subject(owner_governance, subject_id_2.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_2.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 2);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 200, "three": 0, "two": 0
        })
    );
}

#[test(tokio::test)]
// Testear los esppacios de nombre
async fn test_namespace_in_role_1() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0], vec![0], vec![0]],
        vec![],
        true,
        45030,
    )
    .await;
    let evaluator = &nodes[0];
    let owner_governance = &nodes[1];
    let emit_events = &nodes[2];
    let witness_schema = &nodes[3];
    let witness_not_schema = &nodes[4];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![evaluator, emit_events, witness_schema, witness_not_schema],
        "",
    )
    .await;

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
                        "id": evaluator.controller_id(),
                        "name": "KoreNode3"
                    }
                },
                {
                    "op": "add",
                    "path": "/members/3",
                    "value": {
                        "id": witness_schema.controller_id(),
                        "name": "KoreNode4"
                    }
                },
                {
                    "op": "add",
                    "path": "/members/4",
                    "value": {
                        "id": witness_not_schema.controller_id(),
                        "name": "KoreNode5"
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/6",
                    "value": {
                        "namespace": "Spain",
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
                    "path": "/roles/8",
                    "value": {
                        "namespace": "Spain",
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
                        "namespace": "Spain",
                        "role": "WITNESS",
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode4"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/10",
                    "value": {
                        "namespace": "Other",
                        "role": "WITNESS",
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode5"
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
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0="
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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create subject
    let subject_id = create_subject(
        emit_events,
        governance_id.clone(),
        "Example",
        "",
        false,
    )
    .await;
    assert!(subject_id.is_err());

    let subject_id = create_subject(
        emit_events,
        governance_id.clone(),
        "Example",
        "Other",
        false,
    )
    .await;
    assert!(subject_id.is_err());

    // create subject in namespace
    let subject_id = create_subject(
        emit_events,
        governance_id.clone(),
        "Example",
        "Spain",
        true,
    )
    .await
    .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });
    emit_fact(emit_events, subject_id.clone(), json, true)
        .await
        .unwrap();

    let state = emit_events
        .get_signatures(subject_id.clone())
        .await
        .unwrap();

    assert!(state.signatures_eval.unwrap().len() == 2);

    let state = get_subject(owner_governance, subject_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "Spain");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    let state = get_subject(emit_events, subject_id.clone()).await.unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "Spain");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    let state = get_subject(witness_schema, subject_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "Spain");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    assert!(
        witness_not_schema
            .get_subject(subject_id.clone())
            .await
            .is_err()
    );
}

#[test(tokio::test)]
// Testear los esppacios de nombre
async fn test_namespace_in_role_2() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0], vec![0], vec![0]],
        vec![],
        true,
        45040,
    )
    .await;
    let evaluator = &nodes[0];
    let owner_governance = &nodes[1];
    let emit_events = &nodes[2];
    let witness_schema = &nodes[3];
    let witness_not_schema = &nodes[4];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![evaluator, emit_events, witness_schema, witness_not_schema],
        "",
    )
    .await;

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
                        "id": evaluator.controller_id(),
                        "name": "KoreNode3"
                    }
                },
                {
                    "op": "add",
                    "path": "/members/3",
                    "value": {
                        "id": witness_schema.controller_id(),
                        "name": "KoreNode4"
                    }
                },
                {
                    "op": "add",
                    "path": "/members/4",
                    "value": {
                        "id": witness_not_schema.controller_id(),
                        "name": "KoreNode5"
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/6",
                    "value": {
                        "namespace": "Spain.Canary.Tenerife",
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
                    "path": "/roles/8",
                    "value": {
                        "namespace": "Spain",
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
                        "namespace": "Spain.Canary",
                        "role": "WITNESS",
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode4"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/10",
                    "value": {
                        "namespace": "Spain.Canary.Tenerife.LaLaguna",
                        "role": "WITNESS",
                        "schema": {
                            "ID": "Example"
                        },
                        "who": {
                            "NAME": "KoreNode5"
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
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0="
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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create subject
    let subject_id = create_subject(
        emit_events,
        governance_id.clone(),
        "Example",
        "Spain",
        false,
    )
    .await;
    assert!(subject_id.is_err());

    let subject_id = create_subject(
        emit_events,
        governance_id.clone(),
        "Example",
        "Spain.Canary",
        false,
    )
    .await;
    assert!(subject_id.is_err());

    // create subject in namespace
    let subject_id = create_subject(
        emit_events,
        governance_id.clone(),
        "Example",
        "Spain.Canary.Tenerife",
        true,
    )
    .await
    .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });
    emit_fact(emit_events, subject_id.clone(), json, true)
        .await
        .unwrap();

    let state = emit_events
        .get_signatures(subject_id.clone())
        .await
        .unwrap();

    assert!(state.signatures_eval.unwrap().len() == 2);

    let state = get_subject(owner_governance, subject_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "Spain.Canary.Tenerife");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    let state = get_subject(emit_events, subject_id.clone()).await.unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "Spain.Canary.Tenerife");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    let state = get_subject(witness_schema, subject_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "Spain.Canary.Tenerife");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, emit_events.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, emit_events.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    assert!(
        witness_not_schema
            .get_subject(subject_id.clone())
            .await
            .is_err()
    );
}

#[test(tokio::test)]
async fn test_many_schema_in_one_governance() {
    let node =
        create_nodes_and_connections(vec![vec![]], vec![], vec![], true, 45050)
            .await;
    let owner_governance = &node[0];

    let governance_id =
        create_and_authorize_governance(owner_governance, vec![], "").await;

    let mut operations = vec![];

    // create many schemas and policies
    for i in 0..3 {
        let policie = json!({
            "op": "add",
            "path": format!("/policies/{}",i),
            "value": {
                "id":  format!("Example{}", i),
                "approve": { "quorum": { "FIXED": 1 } },
                "evaluate": { "quorum": "MAJORITY" },
                "validate": { "quorum": "MAJORITY" }
            }
        });
        let schema = json!({
            "op": "add",
            "path": format!("/schemas/{}", i),
            "value": {
                "id": format!("Example{}", i),
                "contract": {
                    "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBzdGF0ZS5vbmUgKyBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IHN0YXRlLnR3byArIGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS50aHJlZSA9IHN0YXRlLnRocmVlICsgZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCg=="
                },
                "initial_value": {
                    "one": 1,
                    "two": 1,
                    "three": 1
                }
            }

        });
        operations.push(policie);
        operations.push(schema);
    }

    let json = json!({"Patch": {
        "data": operations
    }});
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_subject(owner_governance, governance_id.clone())
        .await
        .unwrap();

    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, "");
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner_governance.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":owner_governance.controller_id(),"name":"Owner"}],"policies":[{"approve":{"quorum":{"FIXED":1}},"evaluate":{"quorum":"MAJORITY"},"id":"Example0","validate":{"quorum":"MAJORITY"}},{"approve":{"quorum":{"FIXED":1}},"evaluate":{"quorum":"MAJORITY"},"id":"Example1","validate":{"quorum":"MAJORITY"}},{"approve":{"quorum":{"FIXED":1}},"evaluate":{"quorum":"MAJORITY"},"id":"Example2","validate":{"quorum":"MAJORITY"}},{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[{"contract":{"raw":"dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBzdGF0ZS5vbmUgKyBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IHN0YXRlLnR3byArIGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS50aHJlZSA9IHN0YXRlLnRocmVlICsgZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCg=="},"id":"Example0","initial_value":{"one":1,"three":1,"two":1}},{"contract":{"raw":"dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBzdGF0ZS5vbmUgKyBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IHN0YXRlLnR3byArIGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS50aHJlZSA9IHN0YXRlLnRocmVlICsgZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCg=="},"id":"Example1","initial_value":{"one":1,"three":1,"two":1}},{"contract":{"raw":"dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBzdGF0ZS5vbmUgKyBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IHN0YXRlLnR3byArIGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS50aHJlZSA9IHN0YXRlLnRocmVlICsgZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCg=="},"id":"Example2","initial_value":{"one":1,"three":1,"two":1}}],"version":1})
    );
}

#[test(tokio::test)]
// Testear la transferencia de gobernanza
async fn test_transfer_event_governance_1() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45060,
    )
    .await;
    let future_owner = &nodes[0];
    let owner_governance = &nodes[1];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![future_owner],
        "",
    )
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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_transfer(
        owner_governance,
        governance_id.clone(),
        KeyIdentifier::from_str(&future_owner.controller_id()).unwrap(),
        true,
    )
    .await
    .unwrap();

    // Confirm transfer event
    emit_confirm(future_owner, governance_id.clone(), None, true)
        .await
        .unwrap();

    let fake_node = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();
    // add new fake member to governance
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": fake_node,
                        "name": "KoreNode2"
                    }
                }
            ]
    }});

    emit_fact(future_owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_subject(future_owner, governance_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, "");
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, future_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":future_owner.controller_id(),"name":"Owner"},{"id":fake_node,"name":"KoreNode2"}],"policies":[{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[],"version":4})
    );

    let state = get_subject(owner_governance, governance_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, "");
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner_governance.controller_id());
    assert_eq!(state.new_owner, Some(future_owner.controller_id()));
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":owner_governance.controller_id(),"name":"Owner"},{"id": future_owner.controller_id(),"name":"KoreNode1"}],"policies":[{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[],"version":2})
    );
}

#[test(tokio::test)]
// Testear la transferencia de gobernanza, pero el owner se queda como miembro
async fn test_transfer_event_governance_2() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45070,
    )
    .await;
    let future_owner = &nodes[0];
    let owner_governance = &nodes[1];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![future_owner],
        "",
    )
    .await;

    // Auth governance in old owner, in future he will be a normal member and need auth governance for receive a ledger copy.
    owner_governance
        .auth_subject(governance_id.clone(), AuthWitness::None)
        .await
        .unwrap();
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

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_transfer(
        owner_governance,
        governance_id.clone(),
        KeyIdentifier::from_str(&future_owner.controller_id()).unwrap(),
        true,
    )
    .await
    .unwrap();

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert_eq!(
        transfer_data[0].actual_owner,
        owner_governance.controller_id()
    );
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, governance_id.to_string());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert_eq!(
        transfer_data[0].actual_owner,
        owner_governance.controller_id()
    );
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, governance_id.to_string());

    // Confirm transfer event
    emit_confirm(
        future_owner,
        governance_id.clone(),
        Some("Korenode_22".to_owned()),
        true,
    )
    .await
    .unwrap();

    let fake_node = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();
    // add new fake member to governance
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": fake_node,
                        "name": "KoreNode2"
                    }
                }
            ]
    }});

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    emit_fact(future_owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_subject(future_owner, governance_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, "");
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, future_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":future_owner.controller_id(),"name":"Owner"},{"id":fake_node,"name":"KoreNode2"},{"id":owner_governance.controller_id(),"name":"Korenode_22"}],"policies":[{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[],"version":4})
    );

    let state = get_subject(owner_governance, governance_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, "");
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, future_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":future_owner.controller_id(),"name":"Owner"},{"id":fake_node,"name":"KoreNode2"},{"id":owner_governance.controller_id(),"name":"Korenode_22"}],"policies":[{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[],"version":4})
    );
}

#[test(tokio::test)]
// Testear la transferencia de sujeto
async fn test_subject_transfer_event_1() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45080,
    )
    .await;
    let future_owner = &nodes[0];
    let owner_governance = &nodes[1];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![future_owner],
        "",
    )
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
                    "path": "/roles/4",
                    "value": {
                        "namespace": "",
                        "role": "ISSUER",
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
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0="
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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create subject
    let subject_id = create_subject(
        owner_governance,
        governance_id.clone(),
        "Example",
        "",
        true,
    )
    .await
    .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });

    emit_fact(owner_governance, subject_id.clone(), json.clone(), true)
        .await
        .unwrap();

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
        true,
    )
    .await
    .unwrap();

    future_owner
        .update_subject(subject_id.clone())
        .await
        .unwrap();

    let _ = get_subject(future_owner, subject_id.clone()).await.unwrap();

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert_eq!(
        transfer_data[0].actual_owner,
        owner_governance.controller_id()
    );
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id.to_string());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert_eq!(
        transfer_data[0].actual_owner,
        owner_governance.controller_id()
    );
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id.to_string());

    emit_confirm(future_owner, subject_id.clone(), None, true)
        .await
        .unwrap();

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    assert!(
        emit_fact(owner_governance, subject_id.clone(), json.clone(), false)
            .await
            .is_err()
    );

    let json = json!({
        "ModOne": {
            "data": 150,
        }
    });
    emit_fact(future_owner, subject_id.clone(), json.clone(), true)
        .await
        .unwrap();

    let state = get_subject(future_owner, subject_id.clone()).await.unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, future_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({
            "one": 150, "three": 0, "two": 0
        })
    );

    let state = get_subject(owner_governance, subject_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, future_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({
            "one": 150, "three": 0, "two": 0
        })
    );
}

#[test(tokio::test)]
// Testear la transferencia de sujeto, entre dos nodos que no son el owner de la gobernanza
async fn test_subject_transfer_event_2() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        45090,
    )
    .await;

    let owner_governance = &nodes[0];
    let future_owner = &nodes[1];
    let old_owner = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![future_owner, old_owner],
        "",
    )
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
                    "path": "/members/2",
                    "value": {
                        "id": old_owner.controller_id(),
                        "name": "KoreNode2"
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/1",
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
                    "path": "/roles/2",
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
                    "path": "/roles/3",
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
                            "NAME": "KoreNode1"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/4",
                    "value": {
                        "namespace": "",
                        "role": "ISSUER",
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
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0="
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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create subject
    let subject_id =
        create_subject(old_owner, governance_id.clone(), "Example", "", true)
            .await
            .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });

    emit_fact(old_owner, subject_id.clone(), json.clone(), true)
        .await
        .unwrap();

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
        old_owner,
        subject_id.clone(),
        KeyIdentifier::from_str(&future_owner.controller_id()).unwrap(),
        true,
    )
    .await
    .unwrap();

    future_owner
        .update_subject(subject_id.clone())
        .await
        .unwrap();
    let _ = get_subject(future_owner, subject_id.clone()).await.unwrap();

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id.to_string());

    let transfer_data = old_owner.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id.to_string());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id.to_string());

    emit_confirm(future_owner, subject_id.clone(), None, true)
        .await
        .unwrap();

    // El owner no sabe que la transferencia se ha acceptado.
    assert!(
        create_subject(old_owner, governance_id.clone(), "Example", "", false)
            .await
            .is_err()
    );

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    let transfer_data = old_owner.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id.to_string());

    old_owner
        .auth_subject(
            subject_id.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&owner_governance.controller_id())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();

    check_transfer(old_owner, subject_id.clone()).await.unwrap();

    let subject_id_2 =
        create_subject(old_owner, governance_id.clone(), "Example", "", true)
            .await
            .unwrap();

    assert!(
        emit_fact(old_owner, subject_id.clone(), json.clone(), false)
            .await
            .is_err()
    );

    let json = json!({
        "ModOne": {
            "data": 150,
        }
    });
    emit_fact(future_owner, subject_id.clone(), json.clone(), true)
        .await
        .unwrap();

    emit_fact(old_owner, subject_id_2.clone(), json.clone(), true)
        .await
        .unwrap();

    let state = get_subject(future_owner, subject_id.clone()).await.unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, future_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, old_owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({
            "one": 150, "three": 0, "two": 0
        })
    );

    let state = get_subject(owner_governance, subject_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, future_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, old_owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({
            "one": 150, "three": 0, "two": 0
        })
    );

    let state = get_subject(old_owner, subject_id_2.clone()).await.unwrap();
    assert_eq!(state.subject_id, subject_id_2.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, old_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, old_owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 150, "three": 0, "two": 0
        })
    );

    let state = get_subject(owner_governance, subject_id_2.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_2.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, old_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, old_owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 150, "three": 0, "two": 0
        })
    );
}

#[test(tokio::test)]
// Testear la transferencia de sujeto, entre dos nodos que no son el owner de la gobernanza
// Pero el nuevo owner ya tiene el límite y tiene que hacer reject y el otro recupera el sujeto.
async fn test_subject_transfer_event_3() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        45100,
    )
    .await;

    let owner_governance = &nodes[0];
    let future_owner = &nodes[1];
    let old_owner = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![future_owner, old_owner],
        "",
    )
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
                    "path": "/members/2",
                    "value": {
                        "id": old_owner.controller_id(),
                        "name": "KoreNode2"
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/1",
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
                    "path": "/roles/2",
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
                    "path": "/roles/3",
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
                            "NAME": "KoreNode1"
                        }
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/4",
                    "value": {
                        "namespace": "",
                        "role": "ISSUER",
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
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0="
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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create subject
    let subject_id_1 =
        create_subject(old_owner, governance_id.clone(), "Example", "", true)
            .await
            .unwrap();

    let _subject_id_2 = create_subject(
        future_owner,
        governance_id.clone(),
        "Example",
        "",
        true,
    )
    .await
    .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });

    emit_fact(old_owner, subject_id_1.clone(), json.clone(), true)
        .await
        .unwrap();

    // autorizar para recibir copias del nuevo sujeto
    future_owner
        .auth_subject(
            subject_id_1.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&owner_governance.controller_id())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();

    // transfer subject
    emit_transfer(
        old_owner,
        subject_id_1.clone(),
        KeyIdentifier::from_str(&future_owner.controller_id()).unwrap(),
        true,
    )
    .await
    .unwrap();

    future_owner
        .update_subject(subject_id_1.clone())
        .await
        .unwrap();
    let _ = get_subject(future_owner, subject_id_1.clone())
        .await
        .unwrap();

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id_1.to_string());

    let transfer_data = old_owner.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id_1.to_string());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id_1.to_string());

    let request = EventRequest::Confirm(ConfirmRequest {
        subject_id: subject_id_1.clone(),
        name_old_owner: None,
    });
    assert!(future_owner.own_request(request).await.is_err());

    // El owner no sabe que la transferencia se ha acceptado.
    assert!(
        create_subject(old_owner, governance_id.clone(), "Example", "", false)
            .await
            .is_err()
    );

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id_1.to_string());

    let transfer_data = old_owner.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id_1.to_string());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert_eq!(transfer_data[0].actual_owner, old_owner.controller_id());
    assert_eq!(transfer_data[0].new_owner, future_owner.controller_id());
    assert_eq!(transfer_data[0].subject_id, subject_id_1.to_string());

    emit_reject(future_owner, subject_id_1.clone(), true)
        .await
        .unwrap();

    old_owner
        .auth_subject(
            subject_id_1.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&owner_governance.controller_id())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();

    check_transfer(old_owner, subject_id_1.clone())
        .await
        .unwrap();

    let transfer_data = old_owner.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    let transfer_data = future_owner.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    let transfer_data = owner_governance.get_pending_transfers().await.unwrap();
    assert!(transfer_data.is_empty());

    assert!(
        emit_fact(future_owner, subject_id_1.clone(), json.clone(), false)
            .await
            .is_err()
    );

    let json = json!({
        "ModOne": {
            "data": 150,
        }
    });
    emit_fact(old_owner, subject_id_1.clone(), json.clone(), true)
        .await
        .unwrap();

    let state = get_subject(old_owner, subject_id_1.clone()).await.unwrap();
    assert_eq!(state.subject_id, subject_id_1.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, old_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, old_owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({
            "one": 150, "three": 0, "two": 0
        })
    );

    let state = get_subject(owner_governance, subject_id_1.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_1.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, old_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, old_owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 4);
    assert_eq!(
        state.properties,
        json!({
            "one": 150, "three": 0, "two": 0
        })
    );

    let state = get_subject(future_owner, subject_id_1.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_1.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, old_owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, old_owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 3);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
}

#[test(tokio::test)]
async fn test_governance_fail_approve() {
    // Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![],
        vec![],
        false,
        45110,
    )
    .await;
    let node1 = &nodes[0];

    let governance_id =
        create_and_authorize_governance(node1, vec![], "").await;

    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": KeyIdentifier::default(),
                        "name": "KoreNode2"
                    }
                }
    ]}});

    let request_id = emit_fact(node1, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_approve(
        node1,
        governance_id.clone(),
        ApprovalStateRes::RespondedRejected,
        request_id,
        true,
    )
    .await
    .unwrap();

    let state = node1.get_subject(governance_id.clone()).await.unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, node1.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, node1.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({"members":[{"id":node1.controller_id(),"name":"Owner"}],"policies":[{"approve":{"quorum":"MAJORITY"},"evaluate":{"quorum":"MAJORITY"},"id":"governance","validate":{"quorum":"MAJORITY"}}],"roles":[{"namespace":"","role":"WITNESS","schema":{"ID":"governance"},"who":"MEMBERS"},{"namespace":"","role":"EVALUATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"ISSUER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"APPROVER","schema":{"ID":"governance"},"who":{"NAME":"Owner"}},{"namespace":"","role":"VALIDATOR","schema":"ALL","who":{"NAME":"Owner"}},{"namespace":"","role":"WITNESS","schema":"ALL","who":{"NAME":"Owner"}}],"schemas":[],"version":0})
    );
}

#[test(tokio::test)]
async fn test_dynamic_witnesses() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        45120,
    )
    .await;

    let owner_governance = &nodes[0];
    let creator = &nodes[1];
    let witness = &nodes[2];

    let governance_id =
        create_and_authorize_governance(owner_governance, vec![creator], "")
            .await;

    // add member to governance
    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": creator.controller_id(),
                        "name": "KoreNode1"
                    }
                },
                {
                    "op": "add",
                    "path": "/roles/1",
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
                            "NAME": "KoreNode1"
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
                            "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0="
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
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create subject
    let subject_id =
        create_subject(creator, governance_id.clone(), "Example", "", true)
            .await
            .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 100,
        }
    });

    emit_fact(creator, subject_id.clone(), json.clone(), true)
        .await
        .unwrap();

    let json = json!({"Patch": {
            "data": [
                {
                    "op": "add",
                    "path": "/members/1",
                    "value": {
                        "id": witness.controller_id(),
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
                }
            ]
    }});

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    witness
        .auth_subject(governance_id.clone(), AuthWitness::None)
        .await
        .unwrap();

    // emit event to subject
    let json = json!({
        "ModOne": {
            "data": 200,
        }
    });

    emit_fact(creator, subject_id.clone(), json.clone(), true)
        .await
        .unwrap();

        let state = get_subject(owner_governance, subject_id.clone())
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, creator.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, creator.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({
            "one": 200, "three": 0, "two": 0
        })
    );
    let state = get_subject(witness, subject_id.clone())
    .await
    .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, creator.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, creator.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({
            "one": 200, "three": 0, "two": 0
        })
    );

    let state = get_subject(creator, subject_id.clone())
    .await
    .unwrap();
    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, creator.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, creator.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
    state.properties,
    json!({
        "one": 200, "three": 0, "two": 0
    })
    );
}

// verificar copias de la gobernanza entre nodos independientemente del tipo ✅
// verificar copias de los eventos de un sujeto independientemente del tipo  ✅
// si no esta autorizado verificar que no recibe la copia ✅
// revisar la limitación en la creación de sujetos y roles . ✅
// creación de sujetos bajo espacios de nombres ✅
// not members solo funciona para ISSUER❌
// MODIFICAR OWNER DE LA GOBERNANZA ✅
// PROBAR LA TRANSFERENCIA ✅
// CREAR GOVERNANZAS CON NAMESPACES ✅
// estar en la red y recibir copia y no ser mienbro
// probar la manual distribution ✅
// probar cosas entre un emit y un confirm
