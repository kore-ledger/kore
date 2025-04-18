use std::{str::FromStr, time::Duration};

mod common;

use identity::{
    identifier::KeyIdentifier,
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{
    approval::approver::ApprovalStateRes,
    auth::AuthWitness,
    model::request::{ConfirmRequest, EventRequest},
};

use common::{
    check_transfer, create_and_authorize_governance,
    create_nodes_and_connections, create_subject, emit_approve, emit_confirm,
    emit_fact, emit_reject, emit_transfer, get_signatures, get_subject,
};
use serde_json::json;
use test_log::test;

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

    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode2",
                    "key": node2.controller_id()
                }
            ]
        },
        "schemas": {
            "add": [
                {
                    "id": "Example",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgZXZlbnRfcHRyLCBpc19vd25lciwgY29udHJhY3RfbG9naWMpCn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
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
                            "creator": [
                                {
                                    "name": "KoreNode2",
                                    "namespace": [],
                                    "quantity": 2
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

    let state = get_subject(node1, subject_id.clone(), None).await.unwrap();
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

    let state = get_subject(node2, subject_id.clone(), None).await.unwrap();
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
    let json = json!({"members": {
        "add": [
            {
                "name": "KoreNode2",
                "key": intermediary.controller_id()
            },
            {
                "name": "KoreNode3",
                "key": emit_events.controller_id()
            }
        ]
    }});

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_subject(owner_governance, governance_id.clone(), None)
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
        json!({"members":{"KoreNode2":intermediary.controller_id(),"KoreNode3":emit_events.controller_id(),"Owner":owner_governance.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{},"schemas":{},"version":1})
    );

    let state = get_subject(intermediary, governance_id.clone(), None)
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
        json!({"members":{"KoreNode2":intermediary.controller_id(),"KoreNode3":emit_events.controller_id(),"Owner":owner_governance.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{},"schemas":{},"version":1})
    );

    let state = get_subject(emit_events, governance_id.clone(), None)
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
        json!({"members":{"KoreNode2":intermediary.controller_id(),"KoreNode3":emit_events.controller_id(),"Owner":owner_governance.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{},"schemas":{},"version":1})
    );
}

#[test(tokio::test)]
async fn test_many_schema_in_one_governance() {
    let node =
        create_nodes_and_connections(vec![vec![]], vec![], vec![], true, 45020)
            .await;
    let owner_governance = &node[0];

    let governance_id =
        create_and_authorize_governance(owner_governance, vec![], "").await;

    let json = json!({
        "schemas": {
            "add": [
                {
                    "id": "Example1",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgZXZlbnRfcHRyLCBpc19vd25lciwgY29udHJhY3RfbG9naWMpCn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                },
                {
                    "id": "Example2",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgZXZlbnRfcHRyLCBpc19vd25lciwgY29udHJhY3RfbG9naWMpCn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                },
                {
                    "id": "Example3",
                    "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgZXZlbnRfcHRyLCBpc19vd25lciwgY29udHJhY3RfbG9naWMpCn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            ]
        },
    });
    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_subject(owner_governance, governance_id.clone(), None)
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
        json!({"members":{"Owner": owner_governance.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{"Example1":{"evaluate":"majority","validate":"majority"},"Example2":{"evaluate":"majority","validate":"majority"},"Example3":{"evaluate":"majority","validate":"majority"}},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{"Example1":{"creator":[],"evaluator":[],"issuer":{"any":false,"users":[]},"validator":[],"witness":[]},"Example2":{"creator":[],"evaluator":[],"issuer":{"any":false,"users":[]},"validator":[],"witness":[]},"Example3":{"creator":[],"evaluator":[],"issuer":{"any":false,"users":[]},"validator":[],"witness":[]}},"schemas":{"Example1":{"contract":"dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgZXZlbnRfcHRyLCBpc19vd25lciwgY29udHJhY3RfbG9naWMpCn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=","initial_value":{"one":0,"three":0,"two":0}},"Example2":{"contract":"dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgZXZlbnRfcHRyLCBpc19vd25lciwgY29udHJhY3RfbG9naWMpCn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=","initial_value":{"one":0,"three":0,"two":0}},"Example3":{"contract":"dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgZXZlbnRfcHRyLCBpc19vd25lciwgY29udHJhY3RfbG9naWMpCn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=","initial_value":{"one":0,"three":0,"two":0}}},"version":1})
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
        45030,
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
    let json = json!({
    "members": {
        "add": [
            {
                "name": "KoreNode2",
                "key": fake_node
            }
        ]
    }});

    emit_fact(future_owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_subject(future_owner, governance_id.clone(), None)
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
        json!({"members":{"KoreNode2":fake_node, "Owner":future_owner.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{},"schemas":{},"version":4})
    );

    let state = get_subject(owner_governance, governance_id.clone(), None)
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
        json!({"members":{"KoreNode1":future_owner.controller_id(),"Owner":owner_governance.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{},"schemas":{},"version":2})
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
        45040,
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
    let json = json!({
    "members": {
        "add": [
            {
                "name": "KoreNode1",
                "key": future_owner.controller_id()
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
    let json = json!({
    "members": {
        "add": [
            {
                "name": "KoreNode2",
                "key": fake_node
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

    let state = get_subject(future_owner, governance_id.clone(), None)
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
        json!({"members":{"KoreNode2":fake_node,"Korenode_22":owner_governance.controller_id(),"Owner":future_owner.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{},"schemas":{},"version":4})
    );

    let state = get_subject(owner_governance, governance_id.clone(), None)
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
        json!({"members":{"KoreNode2":fake_node,"Korenode_22":owner_governance.controller_id(),"Owner":future_owner.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{},"schemas":{},"version":4})
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
        45050,
    )
    .await;
    let node1 = &nodes[0];

    let governance_id =
        create_and_authorize_governance(node1, vec![], "").await;

    let fake_node = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();

    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": fake_node
                }
            ]
        }
    });

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
        json!({"members":{"Owner":node1.controller_id()},"policies_gov":{"approve":"majority","evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_gov":{"approver":["Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_schema":{},"schemas":{},"version":0})
    );
}

#[test(tokio::test)]
// Varios approvers y todos dicen que sí, se cumple el quorum.
async fn test_governance_manual_many_approvers() {
    // Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        false,
        45060,
    )
    .await;
    let owner = &nodes[0];
    let approver_1 = &nodes[1];
    let approver_2 = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner,
        vec![approver_1, approver_2],
        "",
    )
    .await;

    let json = json!({
        "policies": {
            "governance": {
                "change": {
                    "approve": {
                        "fixed": 100
                    }
                }
            }
        },
        "roles": {
            "governance": {
                "add": {
                    "approver": ["Approver1", "Approver2"]
                }
            }
        },
        "members": {
            "add": [
                {
                    "name": "Approver1",
                    "key": approver_1.controller_id()
                },
                {
                    "name": "Approver2",
                    "key": approver_2.controller_id()
                }
            ]
        }
    });

    let request_id = emit_fact(owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_approve(
        owner,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id,
        true,
    )
    .await
    .unwrap();

    let fake_node = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();

    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": fake_node
                }
            ]
        }
    });

    let request_id = emit_fact(owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_approve(
        owner,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id.clone(),
        true,
    )
    .await
    .unwrap();

    emit_approve(
        approver_1,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id.clone(),
        false,
    )
    .await
    .unwrap();

    emit_approve(
        approver_2,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id.clone(),
        false,
    )
    .await
    .unwrap();

    let state = get_signatures(owner, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 3);
    let state = get_signatures(approver_1, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 3);
    let state = get_signatures(approver_2, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 3);

    let state = get_subject(owner, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"KoreNode1":fake_node,"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":2})
    );
    let state = get_subject(approver_1, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"KoreNode1":fake_node,"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":2})
    );
    let state = get_subject(approver_2, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"KoreNode1":fake_node,"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":2})
    );
}

#[test(tokio::test)]
// Varios approvers y todos dicen que sí, se cumple el quorum. de forma automática.
async fn test_governance_auto_many_approvers() {
    // Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        true,
        45070,
    )
    .await;
    let owner = &nodes[0];
    let approver_1 = &nodes[1];
    let approver_2 = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner,
        vec![approver_1, approver_2],
        "",
    )
    .await;

    let json = json!({
        "policies": {
            "governance": {
                "change": {
                    "approve": {
                        "fixed": 100
                    }
                }
            }
        },
        "roles": {
            "governance": {
                "add": {
                    "approver": ["Approver1", "Approver2"]
                }
            }
        },
        "members": {
            "add": [
                {
                    "name": "Approver1",
                    "key": approver_1.controller_id()
                },
                {
                    "name": "Approver2",
                    "key": approver_2.controller_id()
                }
            ]
        }
    });

    let request_id = emit_fact(owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_approve(
        owner,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id,
        true,
    )
    .await
    .unwrap();

    let fake_node = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();

    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": fake_node
                }
            ]
        }
    });

    let request_id = emit_fact(owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_approve(
        owner,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id.clone(),
        true,
    )
    .await
    .unwrap();

    emit_approve(
        approver_1,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id.clone(),
        false,
    )
    .await
    .unwrap();

    emit_approve(
        approver_2,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id.clone(),
        false,
    )
    .await
    .unwrap();

    let state = get_signatures(owner, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 3);
    let state = get_signatures(approver_1, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 3);
    let state = get_signatures(approver_2, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 3);

    let state = get_subject(owner, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"KoreNode1":fake_node,"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":2})
    );
    let state = get_subject(approver_1, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"KoreNode1":fake_node,"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":2})
    );
    let state = get_subject(approver_2, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"KoreNode1":fake_node,"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":2})
    );
}

#[test(tokio::test)]
// Varios approvers pero uno dice que no y el quorum no se cumple.
async fn test_governance_not_quorum_many_approvers() {
    // Bootstrap ≤- Addressable
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0], vec![0]],
        vec![],
        false,
        45080,
    )
    .await;
    let owner = &nodes[0];
    let approver_1 = &nodes[1];
    let approver_2 = &nodes[2];

    let governance_id = create_and_authorize_governance(
        owner,
        vec![approver_1, approver_2],
        "",
    )
    .await;

    let json = json!({
        "policies": {
            "governance": {
                "change": {
                    "approve": {
                        "fixed": 100
                    }
                }
            }
        },
        "roles": {
            "governance": {
                "add": {
                    "approver": ["Approver1", "Approver2"]
                }
            }
        },
        "members": {
            "add": [
                {
                    "name": "Approver1",
                    "key": approver_1.controller_id()
                },
                {
                    "name": "Approver2",
                    "key": approver_2.controller_id()
                }
            ]
        }
    });

    let request_id = emit_fact(owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_approve(
        owner,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id,
        true,
    )
    .await
    .unwrap();

    let fake_node = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();

    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode1",
                    "key": fake_node
                }
            ]
        }
    });

    let request_id = emit_fact(owner, governance_id.clone(), json, true)
        .await
        .unwrap();

    emit_approve(
        owner,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id.clone(),
        true,
    )
    .await
    .unwrap();

    emit_approve(
        approver_1,
        governance_id.clone(),
        ApprovalStateRes::RespondedAccepted,
        request_id.clone(),
        false,
    )
    .await
    .unwrap();

    emit_approve(
        approver_2,
        governance_id.clone(),
        ApprovalStateRes::RespondedRejected,
        request_id.clone(),
        false,
    )
    .await
    .unwrap();

    let state = get_signatures(owner, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 2);
    let state = get_signatures(approver_1, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 2);
    let state = get_signatures(approver_2, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_appr.unwrap().len(), 2);

    let state = get_subject(owner, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":1})
    );
    let state = get_subject(approver_1, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":1})
    );
    let state = get_subject(approver_2, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.subject_id, governance_id.to_string());
    assert_eq!(state.governance_id, String::default());
    assert_eq!(state.genesis_gov_version, 0);
    assert_eq!(state.namespace, "");
    assert_eq!(state.schema_id, "governance");
    assert_eq!(state.owner, owner.controller_id());
    assert_eq!(state.new_owner, None);
    assert_eq!(state.creator, owner.controller_id());
    assert_eq!(state.active, true);
    assert_eq!(state.sn, 2);
    assert_eq!(
        state.properties,
        json!({"members":{"Approver1":approver_1.controller_id(),"Approver2":approver_2.controller_id(),"Owner":owner.controller_id()},"policies_gov":{"approve":{"fixed":100},"evaluate":"majority","validate":"majority"},"policies_schema":{},"roles_all_schemas":{"evaluator":[{"name":"Owner","namespace":[]}],"issuer":{"any":false,"users":[]},"validator":[{"name":"Owner","namespace":[]}],"witness":[{"name":"Owner","namespace":[]}]},"roles_gov":{"approver":["Approver1","Approver2","Owner"],"evaluator":["Owner"],"issuer":{"any":false,"users":["Owner"]},"validator":["Owner"],"witness":[]},"roles_schema":{},"schemas":{},"version":1})
    );
}

#[test(tokio::test)]
// Se añade un evaluador, se evalua, se le elimina y se vuelve a evaluar.
async fn test_change_roles_gov() {
    let nodes = create_nodes_and_connections(
        vec![vec![]],
        vec![vec![0]],
        vec![],
        true,
        45090,
    )
    .await;
    let eval_node = &nodes[0];
    let owner_governance = &nodes[1];

    let governance_id =
        create_and_authorize_governance(owner_governance, vec![eval_node], "")
            .await;
    // add member to governance
    let json: serde_json::Value = json!({
    "roles": {
        "governance": {
            "add": {
                "evaluator": ["KoreNode1"]
            }
        }
    },
    "members": {
        "add": [
            {
                "name": "KoreNode1",
                "key": eval_node.controller_id()
            }
        ]
    }});

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let fake_node = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();

    let json = json!({
    "members": {
        "add": [
            {
                "name": "KoreNode2",
                "key": fake_node
            }
        ]
    }});

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_signatures(owner_governance, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.signatures_eval.unwrap().len(), 2);
    assert_eq!(state.sn, 2);
    let state = get_signatures(eval_node, governance_id.clone(), Some(2))
        .await
        .unwrap();
    assert_eq!(state.signatures_eval.unwrap().len(), 2);
    assert_eq!(state.sn, 2);

    let json = json!({
    "roles": {
        "governance": {
            "remove": {
                "evaluator": ["KoreNode1"]
            }
        }
    }});

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_signatures(owner_governance, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.signatures_eval.unwrap().len(), 2);
    assert_eq!(state.sn, 3);
    let state = get_signatures(eval_node, governance_id.clone(), Some(3))
        .await
        .unwrap();
    assert_eq!(state.signatures_eval.unwrap().len(), 2);
    assert_eq!(state.sn, 3);

    let fake_node = KeyPair::Ed25519(Ed25519KeyPair::new())
        .key_identifier()
        .to_string();

    let json = json!({
        "members": {
            "add": [
                {
                    "name": "KoreNode3",
                    "key": fake_node
                }
            ]
    }});

    emit_fact(owner_governance, governance_id.clone(), json, true)
        .await
        .unwrap();

    let state = get_signatures(owner_governance, governance_id.clone(), None)
        .await
        .unwrap();
    assert_eq!(state.signatures_eval.unwrap().len(), 1);
    assert_eq!(state.sn, 4);

    let state = get_signatures(eval_node, governance_id.clone(), Some(4))
        .await
        .unwrap();
    assert_eq!(state.signatures_eval.unwrap().len(), 1);
    assert_eq!(state.sn, 4);
}
