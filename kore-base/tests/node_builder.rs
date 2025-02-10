use identity::{
    identifier::{
        derive::{digest::DigestDerivator, KeyDerivator},
        DigestIdentifier, KeyIdentifier,
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{
    config::{Config, ExternalDbConfig, KoreDbConfig},
    model::{
        request::{
            ConfirmRequest, CreateRequest, EventRequest, FactRequest,
            TransferRequest,
        },
        Namespace, ValueWrapper,
    },
    Api,
};
use network::{Config as NetworkConfig, RoutingNode};
use prometheus_client::registry::Registry;
use std::{fs, str::FromStr, time::Duration};
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
) -> Api {
    let keys = KeyPair::Ed25519(Ed25519KeyPair::new());

    let dir = tempfile::tempdir().expect("Can not create temporal directory.");
    let path = dir.path().to_str().unwrap();

    let network_config = NetworkConfig::new(
        node_type,
        vec![listen_address.to_owned()],
        vec![],
        peers,
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
        network: network_config,
        contracts_dir: create_temp_dir(),
        always_accept,
        garbage_collector: Duration::from_secs(500),
        sink: "".to_owned(),
    };

    let mut registry = Registry::default();
    let token = CancellationToken::new();

    Api::new(keys, config, &mut registry, "kore", &token)
        .await
        .unwrap()
}

pub async fn create_nodes_and_connections(
    bootstrap: Vec<Vec<usize>>,
    addressable: Vec<Vec<usize>>,
    ephemeral: Vec<Vec<usize>>,
    always_accept: bool,
    initial_port: u32,
) -> Vec<Api> {
    let mut nodes: Vec<Api> = Vec::new();
    let mut node_addresses = Vec::new();


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

        println!("Bootstrap know: {:?}", peers);

        let node = create_node(
            network::NodeType::Bootstrap,
            &listen_address,
            peers,
            always_accept,
        )
        .await;
        nodes.push(node);
        node_addresses.push(listen_address);
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

        println!("Addressable know: {:?}", peers);

        let node = create_node(
            network::NodeType::Addressable,
            &listen_address,
            peers,
            always_accept,
        )
        .await;
        nodes.push(node);
        node_addresses.push(listen_address);
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

        println!("Ephemeral know: {:?}", peers);

        let node = create_node(
            network::NodeType::Ephemeral,
            &listen_address,
            peers,
            always_accept,
        )
        .await;
        nodes.push(node);
        node_addresses.push(listen_address);
    }

    // print listen and routing each node
    for (i, node) in node_addresses.iter().enumerate() {
        println!("Node {}: listen: {}", nodes[i].peer_id(), node);
    }

    nodes
}

pub async fn create_nodes_massive(
    size_bootstrap: usize,
    size_addressable: usize,
    size_ephemeral: usize,
    initial_port: u32,
    always_accept: bool,
) -> [Vec<(Api, String)>; 3] {
    let mut bootstrap_nodes = Vec::new();
    let mut addressable_nodes = Vec::new();
    let mut ephemeral_nodes = Vec::new();

    // Helper closure to generate node address
    let get_node_address = |index: u32| -> String {
        format!("/ip4/127.0.0.1/tcp/{}", initial_port + index)
    };

    // Create Bootstrap nodes and interconnect them
    for i in 0..size_bootstrap {
        let listen_address = get_node_address(i as u32);

        // Connect to all previously created Bootstrap nodes
        let peers = bootstrap_nodes
            .iter()
            .map(|(node, address): &(Api, String)| RoutingNode {
                peer_id: node.peer_id().clone(),
                address: vec![address.clone()],
            })
            .collect::<Vec<RoutingNode>>();

        let node = create_node(
            network::NodeType::Bootstrap,
            &listen_address,
            peers,
            always_accept,
        )
        .await;

        bootstrap_nodes.push((node, listen_address));
    }

    tokio::time::sleep(Duration::from_secs(8)).await;

    // Create Addressable nodes and connect them to all Bootstrap nodes
    for i in 0..size_addressable {
        let listen_address = get_node_address(size_bootstrap as u32 + i as u32);

        let peers = bootstrap_nodes
            .iter()
            .map(|(node, address)| RoutingNode {
                peer_id: node.peer_id().clone(),
                address: vec![address.clone()],
            })
            .collect();

        let node = create_node(
            network::NodeType::Addressable,
            &listen_address,
            peers,
            always_accept,
        )
        .await;

        addressable_nodes.push((node, listen_address));
    }

    // Create Ephemeral nodes and connect them to all Bootstrap nodes
    for i in 0..size_ephemeral {
        let listen_address = get_node_address(
            (size_bootstrap + size_addressable) as u32 + i as u32,
        );

        let peers = bootstrap_nodes
            .iter()
            .map(|(node, address)| RoutingNode {
                peer_id: node.peer_id().clone(),
                address: vec![address.clone()],
            })
            .collect();

        let node = create_node(
            network::NodeType::Ephemeral,
            &listen_address,
            peers,
            always_accept,
        )
        .await;

        ephemeral_nodes.push((node, listen_address));
    }

    [bootstrap_nodes, addressable_nodes, ephemeral_nodes]
}

/// Crea una governance en `owner_node` y lo autoriza en `other_nodes`.
/// Retorna el `governance_id` generado.
pub async fn create_and_authorize_governance(
    owner_node: &Api,
    other_nodes: &[&Api],
    namespace: &str,
) -> DigestIdentifier {
    let request = EventRequest::Create(CreateRequest {
        governance_id: DigestIdentifier::default(),
        schema_id: "governance".to_owned(),
        namespace: Namespace::from(namespace),
    });
    let data = owner_node.own_request(request).await.unwrap();
    let governance_id = DigestIdentifier::from_str(&data.subject_id).unwrap();

    for node in other_nodes {
        let response = node
            .auth_subject(
                governance_id.clone(),
                kore_base::auth::AuthWitness::One(
                    KeyIdentifier::from_str(&owner_node.controller_id())
                        .unwrap(),
                ),
            )
            .await
            .unwrap();
        assert_eq!(
            response,
            "Ok",
            "Authorization failed in node {:?}",
            node.peer_id()
        );
    }

    governance_id
}

/// Verifica que un governance exista (o tenga al menos un elemento) en varios nodos.
/// Si no está sincronizado, puede hacer reintentos y/o llamar a `update_subject`.
pub async fn wait_for_governance_sync(
    governance_id: DigestIdentifier,
    nodes: &[&Api],
    max_retries: usize,
    sn: u64,
    sleep_sec: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    for attempt in 0..max_retries {
        let mut all_synced = true;
        for node in nodes {
            let govs = node.all_govs(None).await?;
            if govs.is_empty() {
                node.update_subject(governance_id.clone()).await?;
                all_synced = false;
            } else {
                let govs =
                    node.get_subject(governance_id.clone()).await.unwrap();
                if govs.sn != sn {
                    println!("Governance SN mismatch: {} != {}", govs.sn, sn);
                    node.update_subject(governance_id.clone()).await?;
                    all_synced = false;
                } else {
                    println!("Have equal SN: {}", sn);
                }
            }
        }

        if all_synced {
            println!(
                "Governance is synced across all nodes on attempt {}",
                attempt
            );
            return Ok(());
        }

        tokio::time::sleep(Duration::from_secs(sleep_sec)).await;
    }

    Err("Governance is not synced across all nodes".into())
}

pub async fn create_subject(
    node: &Api,
    governance_id: DigestIdentifier,
    schema_id: &str,
    namespace: &str,
) -> Result<DigestIdentifier, Box<dyn std::error::Error>> {
    let request = EventRequest::Create(CreateRequest {
        governance_id,
        schema_id: schema_id.to_owned(),
        namespace: Namespace::from(namespace),
    });
    let data = node.own_request(request).await?;
    Ok(DigestIdentifier::from_str(&data.subject_id)?)
}

pub async fn emit_fact(
    node: &Api,
    subject_id: DigestIdentifier,
    payload_json: serde_json::Value,
    by_pass: Option<bool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = EventRequest::Fact(FactRequest {
        subject_id,
        payload: ValueWrapper(payload_json),
    });
    let response = node.own_request(request).await?;
    println!("emit_fact - response: {:?}", response);
    // state of request
    if by_pass.is_some() {
        return Ok(());
    }
    loop {
        let state = node
            .request_state(
                DigestIdentifier::from_str(&response.request_id).unwrap(),
            )
            .await?;
        println!("emit_fact - status: {:?}", state.status);
        println!("emit_fact - error: {:?}", state.error);
        if state.status == "Finish"
            || state.status == "In Approval"
            || state.status == "Invalid"
        {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

pub async fn emit_transfer(
    node: &Api,
    subject_id: DigestIdentifier,
    new_owner: KeyIdentifier,
    by_pass: Option<bool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let request = EventRequest::Transfer(TransferRequest {
        subject_id,
        new_owner,
    });
    let response = node.own_request(request).await?;
    println!("emit_transfer - response: {:?}", response);
    // state of request
    if by_pass.is_some() {
        return Ok(());
    }
    loop {
        let state = node
            .request_state(
                DigestIdentifier::from_str(&response.request_id).unwrap(),
            )
            .await?;
        println!("emit_transfer - status: {:?}", state.status);
        println!("emit_transfer - error: {:?}", state.error);
        if state.status == "Finish" || state.status == "In Approval" {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

pub async fn emit_confirm(
    node: &Api,
    subject_id: DigestIdentifier,
    by_pass: Option<bool>,
) {
    let request = EventRequest::Confirm(ConfirmRequest { subject_id, name_old_owner: None });
    let response = node.own_request(request).await.unwrap();
    println!("emit_confirm - response: {:?}", response);
    // state of request
    if by_pass.is_some() {
        return;
    }
    loop {
        let state = node
            .request_state(
                DigestIdentifier::from_str(&response.request_id).unwrap(),
            )
            .await
            .unwrap();
        println!("emit_confirm - status: {:?}", state.status);
        println!("emit_confirm - error: {:?}", state.error);
        if state.status == "Finish" || state.status == "In Approval" {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
