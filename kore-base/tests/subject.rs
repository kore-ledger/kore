use std::{str::FromStr, time::Duration};

mod common;

use common::{
    check_transfer, create_and_authorize_governance,
    create_nodes_and_connections, create_subject, emit_confirm, emit_fact,
    emit_reject, emit_transfer, get_signatures, get_subject,
};
use identity::identifier::KeyIdentifier;
use kore_base::{
    auth::AuthWitness,
    model::request::{ConfirmRequest, EventRequest},
};
use serde_json::json;
use test_log::test;

use crate::common::{create_node, node_running};

/*
#[test(tokio::test)]
async fn test_prueba() {
    let listen_address = format!("/memory/46000");

    let (local_db, ext_db, governance_id) = {
        let (owner_governance, local_db, ext_db, token) = create_node(
            network::NodeType::Bootstrap,
            &listen_address,
            vec![],
            true,
            None,
        )
        .await;

        node_running(&owner_governance).await.unwrap();

        let governance_id =
            create_and_authorize_governance(&owner_governance, vec![], "")
                .await;

        // add node bootstrap and ephemeral to governance
        let json = json!({
            "schemas": {
                "add": [
                    {
                        "id": "Example",
                        "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                        "initial_value": {
                            "one": 0,
                            "two": 0,
                            "three": 0
                        }
                    }
                ]
            },
            "roles": {
                "schema":
                    [
                    {
                        "schema_id": "Example",
                        "roles": {
                            "add": {
                                "evaluator": [
                                    {
                                        "name": "Owner",
                                        "namespace": []
                                    }
                                ],
                                "validator": [
                                    {
                                        "name": "Owner",
                                        "namespace": []
                                    }
                                ],
                                "witness": [
                                    {
                                        "name": "Owner",
                                        "namespace": []
                                    }
                                ],
                                "creator": [
                                    {
                                        "name": "Owner",
                                        "namespace": [],
                                        "quantity": 1
                                    }
                                ],
                                "issuer": [
                                    {
                                        "name": "Owner",
                                        "namespace": []
                                    }
                                ]
                            }
                        }
                    }
                ]
            }
        });

        emit_fact(&owner_governance, governance_id.clone(), json, true)
            .await
            .unwrap();

        let subject_id = create_subject(
            &owner_governance,
            governance_id.clone(),
            "Example",
            "",
            true,
        )
        .await
        .unwrap();

        let subjects = owner_governance
            .all_subjs(governance_id.clone(), None, None)
            .await
            .unwrap();
        println!("Mostrando sujetos: {:#?}", subjects);
        token.cancel();
        tokio::time::sleep(Duration::from_secs(3)).await;

        (local_db, ext_db, governance_id.clone())
    };

    let listen_address = format!("/memory/46001");

    let (owner_governance, local_db, ext_db, token) = create_node(
        network::NodeType::Bootstrap,
        &listen_address,
        vec![],
        true,
        Some((local_db, ext_db)),
    )
    .await;

    let subjects = owner_governance
        .all_subjs(governance_id.clone(), None, None)
        .await
        .unwrap();
    
    println!("Mostrando sujetos: {:#?}", subjects);
}
*/

#[test(tokio::test)]
// Testear limitaciones en la creación de sujetos INFINITY - QUANTITY
async fn test_limits_in_subjects() {
    //  Ephemeral -> Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        46000,
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
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode2",
                    "key": emit_events.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode2"
                    ]
                }
            },
            "schema":
                [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "evaluator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "validator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "witness": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "creator": [
                                {
                                    "name": "KoreNode2",
                                    "namespace": [],
                                    "quantity": 1
                                }
                            ],
                            "issuer": [
                                {
                                    "name": "KoreNode2",
                                    "namespace": []
                                }
                            ]
                        }
                    }
                }
            ]
        }
    });

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
    let json = json!({
        "roles": {
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "change": {
                            "creator": [
                                {
                                    "actual_name": "KoreNode2",
                                    "actual_namespace": [],
                                    "new_quantity": "infinity"
                                }
                            ]
                        }
                    }
                }
            ]
        }
    });

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create other subject
    let subject_id_2 =
        create_subject(emit_events, governance_id.clone(), "Example", "", true)
            .await
            .unwrap();

    // now we have two subjects, modify the governance to allow only one
    let json = json!({
        "roles": {
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "change": {
                            "creator": [
                                {
                                    "actual_name": "KoreNode2",
                                    "actual_namespace": [],
                                    "new_quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        }
    });

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

    let state = get_subject(emit_events, subject_id_1.clone(), None)
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

    let state = get_subject(owner_governance, subject_id_1.clone(), None)
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

    let state = get_subject(emit_events, subject_id_2.clone(), None)
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

    let state = get_subject(owner_governance, subject_id_2.clone(), None)
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
        46010,
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
    let json = json!({
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode2", "KoreNode3", "KoreNode4", "KoreNode5"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "KoreNode3",
                            "namespace": ["Spain"]
                        }
                    ],
                    "witness": [
                        {
                            "name": "KoreNode4",
                            "namespace": ["Spain"]
                        },
                        {
                            "name": "KoreNode5",
                            "namespace": ["Other"]
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode2",
                            "namespace": []
                        }
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "evaluator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "validator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "witness": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "creator": [
                                {
                                    "name": "KoreNode2",
                                    "namespace": ["Spain"],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
        "members": {
            "add": [
                {
                    "name": "KoreNode2",
                    "key": emit_events.controller_id()
                },
                {
                    "name": "KoreNode3",
                    "key": evaluator.controller_id()
                },
                {
                    "name": "KoreNode4",
                    "key": witness_schema.controller_id()
                },
                {
                    "name": "KoreNode5",
                    "key": witness_not_schema.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "policies": {
            "schema": [
                {
                    "schema_id": "Example",
                    "policies": {
                        "change": {
                            "evaluate": {
                                "fixed": 10
                            },
                            "validate": {
                                "fixed": 10
                            }
                        }
                    }
                }
            ]
        }
    });

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

    let state = get_signatures(emit_events, subject_id.clone(), None)
        .await
        .unwrap();

    assert!(state.signatures_eval.unwrap().len() == 2);

    let state = get_subject(owner_governance, subject_id.clone(), None)
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

    let state = get_subject(emit_events, subject_id.clone(), None)
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

    let state = get_subject(witness_schema, subject_id.clone(), None)
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
        46020,
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

    let json = json!({
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode2", "KoreNode3", "KoreNode4", "KoreNode5"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "KoreNode3",
                            "namespace": ["Spain"]
                        }
                    ],
                    "witness": [
                        {
                            "name": "KoreNode4",
                            "namespace": ["Spain", "Canary"]
                        },
                        {
                            "name": "KoreNode5",
                            "namespace": ["Spain", "Canary", "Gran Canaria"]
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode2",
                            "namespace": []
                        }
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "evaluator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "validator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "witness": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "creator": [
                                {
                                    "name": "KoreNode2",
                                    "namespace": ["Spain", "Canary", "Tenerife"],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
        "members": {
            "add": [
                {
                    "name": "KoreNode2",
                    "key": emit_events.controller_id()
                },
                {
                    "name": "KoreNode3",
                    "key": evaluator.controller_id()
                },
                {
                    "name": "KoreNode4",
                    "key": witness_schema.controller_id()
                },
                {
                    "name": "KoreNode5",
                    "key": witness_not_schema.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "policies": {
            "schema": [
                {
                    "schema_id": "Example",
                    "policies": {
                        "change": {
                            "evaluate": {
                                "fixed": 10
                            },
                            "validate": {
                                "fixed": 10
                            }
                        }
                    }
                }
            ]
        }
    });

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

    let state = get_signatures(emit_events, subject_id.clone(), None)
        .await
        .unwrap();
    assert!(state.signatures_eval.unwrap().len() == 2);

    let state = get_subject(owner_governance, subject_id.clone(), None)
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

    let state = get_subject(emit_events, subject_id.clone(), None)
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

    let state = get_subject(witness_schema, subject_id.clone(), None)
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
// Testear la transferencia de sujeto
async fn test_subject_transfer_event_1() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        46030,
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
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": future_owner.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode1"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "issuer": [
                        {
                            "name": "KoreNode1",
                            "namespace": []
                        },
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "evaluator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "validator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "witness": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "creator": [
                                {
                                    "name": "KoreNode1",
                                    "namespace": [],
                                    "quantity": "infinity"
                                },
                                {
                                    "name": "Owner",
                                    "namespace": [],
                                    "quantity": "infinity"
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });

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

    let _ = get_subject(future_owner, subject_id.clone(), None)
        .await
        .unwrap();

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

    let state = get_subject(future_owner, subject_id.clone(), None)
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

    let state = get_subject(owner_governance, subject_id.clone(), None)
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
        46040,
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
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": future_owner.controller_id()
                },
                {
                    "name": "KoreNode2",
                    "key": old_owner.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode1", "KoreNode2"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "Owner",
                            "namespace": []
                            }
                    ],
                    "validator": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "witness": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode1",
                            "namespace": []
                        },
                        {
                            "name": "KoreNode2",
                            "namespace": []
                        }
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "creator": [
                                {
                                    "name": "KoreNode1",
                                    "namespace": [],
                                    "quantity": 1
                                },
                                {
                                    "name": "KoreNode2",
                                    "namespace": [],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });

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
    let _ = get_subject(future_owner, subject_id.clone(), None)
        .await
        .unwrap();

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

    let state = get_subject(future_owner, subject_id.clone(), None)
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

    let state = get_subject(owner_governance, subject_id.clone(), None)
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

    let state = get_subject(old_owner, subject_id_2.clone(), None)
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

    let state = get_subject(owner_governance, subject_id_2.clone(), None)
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
        46050,
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
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": future_owner.controller_id()
                },
                {
                    "name": "KoreNode2",
                    "key": old_owner.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode1", "KoreNode2"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "Owner",
                            "namespace": []
                            }
                    ],
                    "validator": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "witness": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode1",
                            "namespace": []
                        },
                        {
                            "name": "KoreNode2",
                            "namespace": []
                        }
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "creator": [
                                {
                                    "name": "KoreNode1",
                                    "namespace": [],
                                    "quantity": 1
                                },
                                {
                                    "name": "KoreNode2",
                                    "namespace": [],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });

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
    let _ = get_subject(future_owner, subject_id_1.clone(), None)
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

    tokio::time::sleep(Duration::from_secs(10)).await;

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

    let state = get_subject(old_owner, subject_id_1.clone(), Some(4))
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

    let state = get_subject(owner_governance, subject_id_1.clone(), Some(4))
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

    let state = get_subject(future_owner, subject_id_1.clone(), Some(3))
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
// Un testigo nuevo reciba las copias de un sujeto que ya va por un sn != 0.
async fn test_dynamic_witnesses_1() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        46060,
    )
    .await;

    let owner_governance = &nodes[0];
    let creator = &nodes[1];
    let witness = &nodes[2];

    let governance_id =
        create_and_authorize_governance(owner_governance, vec![creator], "")
            .await;

    // add member to governance
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": creator.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode1"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "Owner",
                            "namespace": []
                            }
                    ],
                    "validator": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "witness": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode1",
                            "namespace": []
                        },
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "creator": [
                                {
                                    "name": "KoreNode1",
                                    "namespace": [],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });
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

    let json = json!({
    "members": {
        "add": [
            {
                "name": "KoreNode2",
                "key": witness.controller_id()
            }
        ]
    },
    "roles": {
        "governance": {
                "add": {
                    "witness": [
                        "KoreNode2"
                    ]
                }
            },
        "schema": [
            {
                "schema_id": "Example",
                "roles": {
                    "add": {
                        "witness": [
                            {
                                "name": "KoreNode2",
                                "namespace": []
                            }
                        ]
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

    let state = get_subject(owner_governance, subject_id.clone(), None)
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
    let state = get_subject(witness, subject_id.clone(), None)
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

    let state = get_subject(creator, subject_id.clone(), None)
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

#[test(tokio::test)]
// Un testigo nuevo le pide la copia a otro testigo viejo.
async fn test_dynamic_witnesses_2() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0], vec![0]],
        vec![],
        true,
        46070,
    )
    .await;

    let owner_governance = &nodes[0];
    let creator = &nodes[1];
    let witness = &nodes[2];
    let new_witness = &nodes[3];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![creator, witness],
        "",
    )
    .await;

    // add member to governance
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": creator.controller_id()
                },
                {
                    "name": "KoreNode2",
                    "key": witness.controller_id()
                },
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode1", "KoreNode2"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "Owner",
                            "namespace": []
                            }
                    ],
                    "validator": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode1",
                            "namespace": []
                        },
                    ],
                    "witness": [
                        {
                            "name": "KoreNode2",
                            "namespace": []
                        },
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "creator": [
                                {
                                    "name": "KoreNode1",
                                    "namespace": [],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });

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

    let json = json!({
    "members": {
        "add": [
            {
                "name": "KoreNode3",
                "key": new_witness.controller_id()
            }
        ]
    },
    "roles": {
        "governance": {
            "add": {
                "witness": [
                    "KoreNode3"
                ]
            }
        },
        "schema": [
            {
                "schema_id": "Example",
                "roles": {
                    "add": {
                        "witness": [
                            {
                                "name": "KoreNode3",
                                "namespace": []
                            }
                        ]
                    }
                }
            }
        ]
    }});

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    new_witness
        .auth_subject(
            governance_id.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&witness.controller_id()).unwrap(),
            ),
        )
        .await
        .unwrap();

    new_witness
        .auth_subject(
            subject_id.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&witness.controller_id()).unwrap(),
            ),
        )
        .await
        .unwrap();

    new_witness
        .update_subject(governance_id.clone())
        .await
        .unwrap();
    let _ = get_subject(new_witness, governance_id.clone(), None)
        .await
        .unwrap();

    new_witness
        .update_subject(subject_id.clone())
        .await
        .unwrap();

    let state = get_subject(witness, subject_id.clone(), None)
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
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
    let state = get_subject(new_witness, subject_id.clone(), Some(1))
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
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
    let state = get_subject(creator, subject_id.clone(), None)
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
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
    let state = get_subject(owner_governance, subject_id.clone(), None)
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
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
}

#[test(tokio::test)]
// EL Owner_governance es testigo pero no explicito, no recibe copia.
// Un testigo nuevo reciba las copias de un sujeto que ya va por un sn != 0.
async fn test_dynamic_witnesses_explicit_1() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        46080,
    )
    .await;

    let owner_governance = &nodes[0];
    let creator = &nodes[1];
    let witness = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![creator, witness],
        "",
    )
    .await;

    // add member to governance
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": creator.controller_id()
                },
                                {
                    "name": "KoreNode2",
                    "key": witness.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode1", "KoreNode2"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "Owner",
                            "namespace": []
                            }
                    ],
                    "validator": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "witness": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode1",
                            "namespace": []
                        },
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "creator": [
                                {
                                    "name": "KoreNode1",
                                    "namespace": [],
                                    "witnesses": ["KoreNode2"],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });
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

    let state = get_subject(creator, subject_id.clone(), None)
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
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
    let state = get_subject(witness, subject_id.clone(), None)
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
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    owner_governance
        .get_subject(subject_id.clone())
        .await
        .unwrap_err();
}

#[test(tokio::test)]
// 2 sujetos, con diferentes namespace, el owner es testigo no explicito de uno,
// el otro tiene un testigo explicito pero no es el owner
// witness recibe la copia del testigo 1 y owner del 2.
async fn test_dynamic_witnesses_explicit_2() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        46090,
    )
    .await;

    let owner_governance = &nodes[0];
    let creator = &nodes[1];
    let witness = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![creator, witness],
        "",
    )
    .await;

    // add member to governance
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": creator.controller_id()
                },
                                {
                    "name": "KoreNode2",
                    "key": witness.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode1", "KoreNode2"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "Owner",
                            "namespace": []
                            }
                    ],
                    "validator": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "witness": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode1",
                            "namespace": []
                        },
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "creator": [
                                {
                                    "name": "KoreNode1",
                                    "namespace": [],
                                    "witnesses": ["KoreNode2"],
                                    "quantity": 1
                                },
                                {
                                    "name": "KoreNode1",
                                    "namespace": ["Spain"],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    // create subject
    let subject_id_1 =
        create_subject(creator, governance_id.clone(), "Example", "", true)
            .await
            .unwrap();

    let subject_id_2 = create_subject(
        creator,
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

    emit_fact(creator, subject_id_1.clone(), json.clone(), true)
        .await
        .unwrap();

    emit_fact(creator, subject_id_2.clone(), json.clone(), true)
        .await
        .unwrap();

    let state = get_subject(creator, subject_id_1.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_1.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, creator.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, creator.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
    let state = get_subject(witness, subject_id_1.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_1.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, creator.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, creator.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    owner_governance
        .get_subject(subject_id_1.clone())
        .await
        .unwrap_err();

    let state = get_subject(creator, subject_id_2.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_2.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "Spain");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, creator.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, creator.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
    let state = get_subject(owner_governance, subject_id_2.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, subject_id_2.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "Spain");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, creator.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, creator.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    witness.get_subject(subject_id_2.clone()).await.unwrap_err();
}

#[test(tokio::test)]
// Un Testigo implicito le pide la copia a otro explicito, pero no se la da
async fn test_dynamic_witnesses_explicit_3() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        46100,
    )
    .await;

    let owner_governance = &nodes[0];
    let creator = &nodes[1];
    let witness = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner_governance,
        vec![creator, witness],
        "",
    )
    .await;

    // add member to governance
    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": creator.controller_id()
                },
                                {
                    "name": "KoreNode2",
                    "key": witness.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "governance": {
                "add": {
                    "witness": [
                        "KoreNode1", "KoreNode2"
                    ]
                }
            },
            "all_schemas": {
                "add": {
                    "evaluator": [
                        {
                            "name": "Owner",
                            "namespace": []
                            }
                    ],
                    "validator": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "witness": [
                        {
                            "name": "Owner",
                            "namespace": []
                        }
                    ],
                    "issuer": [
                        {
                            "name": "KoreNode1",
                            "namespace": []
                        },
                    ]
                }
            },
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "creator": [
                                {
                                    "name": "KoreNode1",
                                    "namespace": [],
                                    "witnesses": ["KoreNode2"],
                                    "quantity": 1
                                },
                            ]
                        }
                    }
                }
            ]
        },
    });
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

    let state = get_subject(creator, subject_id.clone(), None)
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
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );
    let state = get_subject(witness, subject_id.clone(), None)
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
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 100, "three": 0, "two": 0
        })
    );

    owner_governance
        .get_subject(subject_id.clone())
        .await
        .unwrap_err();

    owner_governance
        .auth_subject(
            subject_id.clone(),
            AuthWitness::One(
                KeyIdentifier::from_str(&witness.controller_id()).unwrap(),
            ),
        )
        .await
        .unwrap();
    owner_governance
        .update_subject(subject_id.clone())
        .await
        .unwrap();

    owner_governance
        .get_subject(subject_id.clone())
        .await
        .unwrap_err();
}

#[test(tokio::test)]
// Un testigo nuevo le pide la copia a otro testigo viejo.
async fn test_no_subject_validator() {
    let nodes =
        create_nodes_and_connections(vec![vec![]], vec![], vec![], true, 46110)
            .await;

    let owner_governance = &nodes[0];

    let governance_id =
        create_and_authorize_governance(owner_governance, vec![], "").await;

    // add member to governance
    let json = json!({
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "creator": [
                                {
                                    "name": "Owner",
                                    "namespace": [],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });

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
        .unwrap_err();
}

#[test(tokio::test)]
// Un testigo nuevo le pide la copia a otro testigo viejo.
async fn test_no_subject_evaluator() {
    let nodes =
        create_nodes_and_connections(vec![vec![]], vec![], vec![], true, 46120)
            .await;

    let owner_governance = &nodes[0];

    let governance_id =
        create_and_authorize_governance(owner_governance, vec![], "").await;

    // add member to governance
    let json = json!({
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "validator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "creator": [
                                {
                                    "name": "Owner",
                                    "namespace": [],
                                    "quantity": 1
                                }
                            ],
                            "issuer": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });

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

    let state = get_subject(owner_governance, subject_id.clone(), None)
        .await
        .unwrap();

    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, owner_governance.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 1);
    assert_eq!(
        state.properties,
        json!({
            "one": 0, "three": 0, "two": 0
        })
    );
}

#[test(tokio::test)]
// Un testigo nuevo le pide la copia a otro testigo viejo.
async fn test_no_subject_issuer() {
    let nodes =
        create_nodes_and_connections(vec![vec![]], vec![], vec![], true, 46130)
            .await;

    let owner_governance = &nodes[0];

    let governance_id =
        create_and_authorize_governance(owner_governance, vec![], "").await;

    // add member to governance
    let json = json!({
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
        "roles": {
            "schema": [
                {
                    "schema_id": "Example",
                    "roles": {
                        "add": {
                            "validator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "creator": [
                                {
                                    "name": "Owner",
                                    "namespace": [],
                                    "quantity": 1
                                }
                            ]
                        }
                    }
                }
            ]
        },
    });

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

    let state = get_subject(owner_governance, subject_id.clone(), None)
        .await
        .unwrap();

    assert_eq!(state.subject_id, subject_id.to_string());
    assert_eq!(state.governance_id, governance_id.to_string());
    assert_eq!(state.genesis_gov_version, 1);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "Example");
    assert_eq!(state.owner, owner_governance.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner_governance.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 0);
    assert_eq!(
        state.properties,
        json!({
            "one": 0, "three": 0, "two": 0
        })
    );
}
