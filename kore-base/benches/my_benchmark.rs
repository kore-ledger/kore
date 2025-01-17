use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use identity::{
    identifier::derive::{digest::DigestDerivator, KeyDerivator},
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use kore_base::model::request::CreateRequest;
use kore_base::model::request::EventRequest;
use kore_base::{
    config::{Config, ExternalDbConfig, KoreDbConfig},
    Api,
};
use network::{Config as NetworkConfig, RoutingNode};
use prometheus_client::registry::Registry;
use std::{fs, time::Duration};
use tokio::runtime::Runtime;
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

async fn governance_copy_benchmark() {
    let node1 = create_node(
        network::NodeType::Bootstrap,
        "/ip4/127.0.0.1/tcp/4500",
        vec![],
        true,
    )
    .await;

    let request = EventRequest::Create(CreateRequest {
        governance_id: Default::default(),
        schema_id: "governance".to_owned(),
        namespace: Default::default(),
    });

    let data = node1.own_request(request).await.unwrap();
    loop {
        let response = node1
            .request_state(data.request_id.parse().unwrap())
            .await.unwrap();
        if response.status == "Finish" {
            break;
        }
    }
}

fn benchmark_governance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("governance copy benchmark", |b| {
        b.to_async(&rt).iter(governance_copy_benchmark);
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(100)
        .warm_up_time(std::time::Duration::new(5, 0));
    targets = benchmark_governance
}

criterion_main!(benches);
