use identity::{
    identifier::{
        DigestIdentifier, KeyIdentifier,
        derive::{KeyDerivator, digest::DigestDerivator},
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::{
    approval::approver::ApprovalStateRes,
    config::{Config, ExternalDbConfig, KoreDbConfig}, helpers::db::common::{SignaturesInfo, SubjectInfo}, model::{
        request::{
            ConfirmRequest, CreateRequest, EventRequest, FactRequest,
            RejectRequest, TransferRequest,
        }, Namespace, ValueWrapper
    }, Api
};
use network::{Config as NetworkConfig, MonitorNetworkState, RoutingNode};
use prometheus_client::registry::Registry;
use std::{collections::BTreeMap, fs, str::FromStr, time::Duration};
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
        sink: BTreeMap::new(),
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

        println!("Addressable know: {:?}", peers);

        let node = create_node(
            network::NodeType::Addressable,
            &listen_address,
            peers,
            always_accept,
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

        println!("Ephemeral know: {:?}", peers);

        let node = create_node(
            network::NodeType::Ephemeral,
            &listen_address,
            peers,
            always_accept,
        )
        .await;

        node_running(&node).await.unwrap();
        nodes.push(node);
    }

    nodes
}

#[allow(dead_code)]
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
    let get_node_address =
        |index: u32| -> String { format!("/memory/{}", initial_port + index) };

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
    wait_request(owner_node, DigestIdentifier::from_str(&data.request_id).unwrap()).await.unwrap();

    for node in other_nodes {
        node
            .auth_subject(
                governance_id.clone(),
                kore_base::auth::AuthWitness::One(
                    KeyIdentifier::from_str(&owner_node.controller_id())
                        .unwrap(),
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
    println!("emit_fact - response: {:?}", response);
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
    sn: Option<u64>
) -> Result<SubjectInfo, Box<dyn std::error::Error>> {
    loop {
        if let Ok(state) =  node
            .get_subject(subject_id.clone())
            .await {
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
    sn: Option<u64>
) -> Result<SignaturesInfo, Box<dyn std::error::Error>> {
    loop {
        if let Ok(state) =  node
            .get_signatures(subject_id.clone())
            .await {
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
    request_id: DigestIdentifier
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        if let Ok(state) =  node
            .request_state(request_id.clone())
            .await {
                if state.status == "Finish"
                    || state.status == "In Approval"
                    || state.status == "Invalid"
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

pub async fn node_running(node: &Api) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        if let Ok(state) =  node
            .get_network_state()
            .await {
                if let MonitorNetworkState::Running = state {
                    break;
                }
            }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

pub async fn check_transfer(node: &Api,subject_id: DigestIdentifier) -> Result<(),  Box<dyn std::error::Error>> {
    node.check_transfer(subject_id.clone()).await.unwrap();

    loop {
        let transfer_data = node.get_pending_transfers().await.unwrap();
        if transfer_data.iter().find(|x| x.subject_id == subject_id.to_string()).is_none() {
            break;
        }
        
        tokio::time::sleep(Duration::from_secs(1)).await
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
    println!("emit_fact - response: {:?}", response);
    // state of request
    if !wait_request_state {
        return Ok(());
    }

    let request_id = DigestIdentifier::from_str(&response.request_id).unwrap();
    wait_request(node, request_id.clone()).await.unwrap();

    Ok(())
}

pub async fn emit_approve(
    node: &Api,
    governance_id: DigestIdentifier,
    res: ApprovalStateRes,
    request_id: DigestIdentifier,
    wait_request_state: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    node
    .approve(governance_id.clone(), res)
    .await
    .unwrap();
    
    // state of request
    if !wait_request_state {
        return Ok(());
    }
    
    loop {
        if let Ok(state) =  node
            .request_state(request_id.clone())
            .await {
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
) -> Result<(), Box<dyn std::error::Error>>{
    let request = EventRequest::Confirm(ConfirmRequest {
        subject_id,
        name_old_owner: new_name,
    });
    let response = node.own_request(request).await?;
    println!("emit_fact - response: {:?}", response);
    // state of request
    if !wait_request_state {
        return Ok(());
    }

    let request_id = DigestIdentifier::from_str(&response.request_id).unwrap();
    wait_request(node, request_id.clone()).await.unwrap();

    Ok(())
}

pub async fn emit_reject(
    node: &Api,
    subject_id: DigestIdentifier,
    wait_request_state: bool,
) -> Result<(), Box<dyn std::error::Error>>{
    let request = EventRequest::Reject(RejectRequest { subject_id });
    let response = node.own_request(request).await?;
    println!("emit_fact - response: {:?}", response);
    // state of request
    if !wait_request_state {
        return Ok(());
    }

    let request_id = DigestIdentifier::from_str(&response.request_id).unwrap();
    wait_request(node, request_id.clone()).await.unwrap();

    Ok(())
}
