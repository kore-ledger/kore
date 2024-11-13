// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later
#![recursion_limit = "256"]
pub mod config;
pub mod error;

mod approval;
mod db;
mod distribution;
mod evaluation;
mod governance;
mod helpers;
mod local_db;
mod model;
mod node;
mod query;
mod request;
mod subject;
pub(crate) mod system;
mod validation;

use actor::ActorRef;
use async_std::sync::RwLock;
use config::Config as KoreBaseConfig;
use error::Error;
use governance::json_schema::JsonSchema;
use governance::schema;
use governance::{init::init_state, Governance};
use helpers::network::*;
use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
use identity::keys::KeyPair;
use intermediary::Intermediary;
use model::event::Event;
use model::request::*;
use model::signature::*;
use model::HashId;
use model::ValueWrapper;
use network::{Monitor, NetworkWorker, PeerId};
use node::{Node, NodeMessage, NodeResponse, SubjectsTypes};
use once_cell::sync::OnceCell;
use prometheus_client::registry::Registry;
use query::Query;
use request::{RequestHandler, RequestHandlerResponse};
use subject::{Subject, SubjectMessage, SubjectResponse};
use system::system;
use tokio_util::sync::CancellationToken;
use validation::{Validation, ValidationInfo, ValidationMessage};

use lazy_static::lazy_static;

use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    /// The digest derivator for the system.
    pub static ref DIGEST_DERIVATOR: Mutex<DigestDerivator> = Mutex::new(DigestDerivator::Blake3_256);
    /// The key derivator for the system.
    pub static ref KEY_DERIVATOR: Mutex<KeyDerivator> = Mutex::new(KeyDerivator::Ed25519);

    pub static ref CONTRACTS: RwLock<HashMap<String, Vec<u8>>> = {
        let contracts = HashMap::new();

        RwLock::new(contracts)
    };
}

static GOVERNANCE: OnceCell<RwLock<JsonSchema>> = OnceCell::new();

pub struct Api {
    peer_id: String,
    controller_id: String,
    request: ActorRef<RequestHandler>,
    node: ActorRef<Node>,
    query: ActorRef<Query>
}

impl Api {
    /// Creates a new `Api`.
    pub async fn new(
        keys: KeyPair,
        config: KoreBaseConfig,
        registry: &mut Registry,
        password: &str,
        token: &CancellationToken
    ) -> Result<Self, Error> {
        let schema = JsonSchema::compile(&schema())?;

        if let Err(_e) = GOVERNANCE.set(RwLock::new(schema)) {
            return Err(Error::JSONSChema("An error occurred with the governance schema, it could not be initialized globally".to_owned()));
        };

        let system = match system(config.clone(), password, Some(token.clone())).await {
            Ok(sys) => sys,
            Err(e) => todo!(),
        };

        let node = Node::new(&keys).unwrap();
        let node_actor = match system.create_root_actor("node", node).await {
            Ok(actor) => actor,
            Err(e) => todo!(),
        };

        let request = RequestHandler::new(keys.key_identifier());
        let request_actor =
            match system.create_root_actor("request", request).await {
                Ok(actor) => actor,
                Err(e) => todo!(),
            };

        let query = Query::new(keys.key_identifier());
        let query_actor = match system.create_root_actor("query", query).await {
            Ok(actor) => actor,
            Err(e) => todo!(),
        };

        let newtork_monitor = Monitor;
        let newtork_monitor_actor = system.create_root_actor("network_monitor", newtork_monitor)
        .await
        .unwrap();

        let mut worker: NetworkWorker<NetworkMessage> = NetworkWorker::new(
            registry,
            keys.clone(),
            config.network.clone(),
            Some(newtork_monitor_actor),
            config.key_derivator.clone(),
            token.clone(),
        )
        .unwrap();

    // Create worker
    let service = Intermediary::new(
        worker.service().sender().clone(),
        KeyDerivator::Ed25519,
        system.clone(),
    );

    let peer_id = worker.local_peer_id().to_string();

    worker.add_helper_sender(service.service().sender());

    system.add_helper("NetworkIntermediary", service).await;

    tokio::spawn(async move {
        let _ = worker.run().await;
    });

        Ok(Self {
            controller_id: keys.key_identifier().to_string(),
            peer_id,
            request: request_actor,
            node: node_actor,
            query: query_actor
        })
    }

    pub fn peer_id(&self) -> String {
        self.peer_id.clone()
    }

    pub fn controller_id(&self) -> String {
        self.controller_id.clone()
    }

    /// Request from issuer.
    pub async fn external_request(
        &self,
        event: Signed<EventRequest>,
    ) -> Result<RequestHandlerResponse, Error> {
        /*
        self.request
            .ask(RequestHandlerCommand::StartRequest(event))
            .await
            .map_err(|e| Error::Actor(e.to_string()))
         */
        todo!()
    }

    /// Own request.
    pub async fn own_request(
        &self,
        event: EventRequest,
    ) -> Result<RequestHandlerResponse, Error> {
        todo!()
    }

    // TODO TODAS LAS REQUEST encoladas
    // TODO TODAS LAS REQUEST Que se está realizando.
    // TODO Información de una determinada request.
    // TODO Todas las request para un determinado sujeto.
}
