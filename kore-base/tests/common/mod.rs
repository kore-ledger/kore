use identity::{
    identifier::{
        DigestIdentifier, KeyIdentifier,
        derive::{KeyDerivator, digest::DigestDerivator},
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{
    Api,
    approval::approver::ApprovalStateRes,
    config::{Config, ExternalDbConfig, KoreDbConfig, SinkAuth},
    helpers::db::common::{SignaturesInfo, SubjectInfo},
    model::{
        Namespace, ValueWrapper,
        request::{
            ConfirmRequest, CreateRequest, EventRequest, FactRequest,
            RejectRequest, TransferRequest,
        },
    },
};
use network::{Config as NetworkConfig, MonitorNetworkState, RoutingNode};
use prometheus_client::registry::Registry;
use std::{fs, str::FromStr, time::Duration};
use tokio::task::JoinHandle;
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

pub async fn create_node(
    node_type: network::NodeType,
    listen_address: &str,
    peers: Vec<RoutingNode>,
    always_accept: bool,
    paths: Option<(String, String)>,
) -> (Api, String, String, CancellationToken, Vec<JoinHandle<()>>) {
    let keys = KeyPair::Ed25519(Ed25519KeyPair::new());

    let (local_db, ext_db) = if let Some((local_db, ext_db)) = paths {
        (local_db, ext_db)
    } else {
        let dir =
            tempfile::tempdir().expect("Can not create temporal directory.");
        let local_db = dir.path().to_str().unwrap();

        let dir =
            tempfile::tempdir().expect("Can not create temporal directory.");
        let ext_db = dir.path().to_str().unwrap();
        (local_db.to_owned(), ext_db.to_owned())
    };

    let network_config = NetworkConfig::new(
        node_type,
        vec![listen_address.to_owned()],
        vec![],
        peers,
    );

    let config = Config {
        key_derivator: KeyDerivator::Ed25519,
        digest_derivator: DigestDerivator::Blake3_256,
        kore_db: KoreDbConfig::build(&local_db),
        external_db: ExternalDbConfig::build(&ext_db),
        network: network_config,
        contracts_dir: create_temp_dir(),
        always_accept,
        garbage_collector: Duration::from_secs(500),
    };

    let mut registry = Registry::default();
    let token = CancellationToken::new();

    let (api, runners) = Api::build(
        keys,
        config,
        SinkAuth::default(),
        &mut registry,
        "kore",
        &token,
    )
    .await
    .unwrap();

    (api, local_db, ext_db, token.clone(), runners)
}

pub async fn create_nodes_and_connections(
    bootstrap: Vec<Vec<usize>>,
    addressable: Vec<Vec<usize>>,
    ephemeral: Vec<Vec<usize>>,
    always_accept: bool,
    initial_port: u32,
) -> Vec<Api> {
    let mut nodes: Vec<Api> = Vec::new();

    let get_node_address = |index: usize| -> String {
        format!("/memory/{}", initial_port + index as u32)
    };

    // Create Bootstrap nodes
    for (i, connections) in bootstrap.iter().enumerate() {
        let listen_address = get_node_address(i);
        let peers = connections
            .iter()
            .map(|&peer_idx| RoutingNode {
                peer_id: nodes[peer_idx].peer_id().clone(),
                address: vec![get_node_address(peer_idx)],
            })
            .collect();

        let (node, ..) = create_node(
            network::NodeType::Bootstrap,
            &listen_address,
            peers,
            always_accept,
            None,
        )
        .await;

        node_running(&node).await.unwrap();
        nodes.push(node);
    }

    tokio::time::sleep(Duration::from_secs(5)).await;

    for (i, connections) in addressable.iter().enumerate() {
        let listen_address = get_node_address(bootstrap.len() + i);
        let peers = connections
            .iter()
            .map(|&peer_idx| RoutingNode {
                peer_id: nodes[peer_idx].peer_id().clone(),
                address: vec![get_node_address(peer_idx)],
            })
            .collect();

        let (node, ..) = create_node(
            network::NodeType::Addressable,
            &listen_address,
            peers,
            always_accept,
            None,
        )
        .await;

        node_running(&node).await.unwrap();
        nodes.push(node);
    }

    for (i, connections) in ephemeral.iter().enumerate() {
        let listen_address =
            get_node_address(bootstrap.len() + addressable.len() + i);
        let peers = connections
            .iter()
            .map(|&peer_idx| RoutingNode {
                peer_id: nodes[peer_idx].peer_id().clone(),
                address: vec![get_node_address(peer_idx)],
            })
            .collect();

        let (node, ..) = create_node(
            network::NodeType::Ephemeral,
            &listen_address,
            peers,
            always_accept,
            None,
        )
        .await;

        node_running(&node).await.unwrap();
        nodes.push(node);
    }

    nodes
}

/// Crea una governance en `owner_node` y lo autoriza en `other_nodes`.
/// Retorna el `governance_id` generado.
/// Correcto
pub async fn create_and_authorize_governance(
    owner_node: &Api,
    other_nodes: Vec<&Api>,
    namespace: &str,
) -> DigestIdentifier {
    let request = EventRequest::Create(CreateRequest {
        name: Some("Governance Tests".to_owned()),
        description: Some("A description for Governance Tests".to_owned()),
        governance_id: DigestIdentifier::default(),
        schema_id: "governance".to_owned(),
        namespace: Namespace::from(namespace),
    });
    let data = owner_node.own_request(request).await.unwrap();
    let governance_id = DigestIdentifier::from_str(&data.subject_id).unwrap();
    wait_request(
        owner_node,
        DigestIdentifier::from_str(&data.request_id).unwrap(),
    )
    .await
    .unwrap();

    for node in other_nodes {
        node.auth_subject(
            governance_id.clone(),
            kore_base::auth::AuthWitness::One(
                KeyIdentifier::from_str(&owner_node.controller_id()).unwrap(),
            ),
        )
        .await
        .unwrap();
    }

    governance_id
}

pub async fn create_subject(
    node: &Api,
    governance_id: DigestIdentifier,
    schema_id: &str,
    namespace: &str,
    wait_request_state: bool,
) -> Result<DigestIdentifier, Box<dyn std::error::Error>> {
    let request = EventRequest::Create(CreateRequest {
        name: Some("A Subject".to_owned()),
        description: Some("A description for Subject".to_owned()),
        governance_id,
        schema_id: schema_id.to_owned(),
        namespace: Namespace::from(namespace),
    });
    let response = node.own_request(request).await?;
    let subject_id = DigestIdentifier::from_str(&response.subject_id).unwrap();

    if !wait_request_state {
        return Ok(subject_id);
    }

    let request_id = DigestIdentifier::from_str(&response.request_id).unwrap();
    wait_request(node, request_id.clone()).await.unwrap();

    Ok(subject_id)
}

pub async fn emit_fact(
    node: &Api,
    subject_id: DigestIdentifier,
    payload_json: serde_json::Value,
    wait_request_state: bool,
) -> Result<DigestIdentifier, Box<dyn std::error::Error>> {
    let request = EventRequest::Fact(FactRequest {
        subject_id,
        payload: ValueWrapper(payload_json),
    });

    let response = node.own_request(request).await?;
    // state of request
    let request_id = DigestIdentifier::from_str(&response.request_id).unwrap();

    if !wait_request_state {
        return Ok(request_id);
    }

    wait_request(node, request_id.clone()).await.unwrap();

    Ok(request_id)
}

pub async fn get_subject(
    node: &Api,
    subject_id: DigestIdentifier,
    sn: Option<u64>,
) -> Result<SubjectInfo, Box<dyn std::error::Error>> {
    loop {
        if let Ok(state) = node.get_subject(subject_id.clone()).await {
            if let Some(sn) = sn {
                if sn == state.sn {
                    return Ok(state);
                }
            } else {
                return Ok(state);
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub async fn get_signatures(
    node: &Api,
    subject_id: DigestIdentifier,
    sn: Option<u64>,
) -> Result<SignaturesInfo, Box<dyn std::error::Error>> {
    loop {
        if let Ok(state) = node.get_signatures(subject_id.clone()).await {
            if let Some(sn) = sn {
                if sn == state.sn {
                    return Ok(state);
                }
            } else {
                return Ok(state);
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub async fn wait_request(
    node: &Api,
    request_id: DigestIdentifier,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        if let Ok(state) = node.request_state(request_id.clone()).await {
            if state.status == "Finish"
                || state.status == "In Approval"
                || state.status == "Invalid"
                || state.status == "Abort"
            {
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Segundo para que la información se escriba en el sumidero
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
}

pub async fn node_running(
    node: &Api,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        if let Ok(state) = node.get_network_state().await {
            if let MonitorNetworkState::Running = state {
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn check_transfer(
    node: &Api,
    subject_id: DigestIdentifier,
) -> Result<(), Box<dyn std::error::Error>> {
    node.check_transfer(subject_id.clone()).await.unwrap();

    let mut count = 0;
    loop {
        if count == 3 {
            node.update_subject(subject_id.clone()).await.unwrap();
            count = 0;
        }

        let transfer_data = node.get_pending_transfers().await.unwrap();
        if transfer_data
            .iter()
            .find(|x| x.subject_id == subject_id.to_string())
            .is_none()
        {
            break;
        }

        count += 1;
        tokio::time::sleep(Duration::from_secs(2)).await
    }

    Ok(())
}

pub async fn emit_transfer(
    node: &Api,
    subject_id: DigestIdentifier,
    new_owner: KeyIdentifier,
    wait_request_state: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = EventRequest::Transfer(TransferRequest {
        subject_id,
        new_owner,
    });

    let response = node.own_request(request).await?;
    // state of request
    if !wait_request_state {
        return Ok(());
    }

    let request_id = DigestIdentifier::from_str(&response.request_id).unwrap();
    wait_request(node, request_id.clone()).await.unwrap();

    Ok(())
}

#[allow(dead_code)]
pub async fn emit_approve(
    node: &Api,
    governance_id: DigestIdentifier,
    res: ApprovalStateRes,
    request_id: DigestIdentifier,
    wait_request_state: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    node.approve(governance_id.clone(), res).await.unwrap();

    // state of request
    if !wait_request_state {
        return Ok(());
    }

    loop {
        if let Ok(state) = node.request_state(request_id.clone()).await {
            if state.status == "Finish"
                || state.status == "In Approval"
                || state.status == "Invalid"
            {
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

pub async fn emit_confirm(
    node: &Api,
    subject_id: DigestIdentifier,
    new_name: Option<String>,
    wait_request_state: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = EventRequest::Confirm(ConfirmRequest {
        subject_id,
        name_old_owner: new_name,
    });
    let response = node.own_request(request).await?;
    // state of request
    if !wait_request_state {
        return Ok(());
    }

    let request_id = DigestIdentifier::from_str(&response.request_id).unwrap();
    wait_request(node, request_id.clone()).await.unwrap();

    Ok(())
}

#[allow(dead_code)]
pub async fn emit_reject(
    node: &Api,
    subject_id: DigestIdentifier,
    wait_request_state: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = EventRequest::Reject(RejectRequest { subject_id });
    let response = node.own_request(request).await?;
    // state of request
    if !wait_request_state {
        return Ok(());
    }

    let request_id = DigestIdentifier::from_str(&response.request_id).unwrap();
    wait_request(node, request_id.clone()).await.unwrap();

    Ok(())
}
