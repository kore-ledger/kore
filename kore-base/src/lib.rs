// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later
#![recursion_limit = "256"]
pub mod error;
pub mod config;

mod approval;
mod db;
mod distribution;
mod evaluation;
mod governance;
mod helpers;
mod model;
mod node;
mod request;
mod subject;
mod validation;
mod query;
mod local_db;
pub(crate) mod system;

use actor::{ActorRef, ActorSystem, SystemRef};
use async_std::sync::RwLock;
use config::{Config as KoreBaseConfig, DbConfig};
use db::Database;
use error::Error;
use governance::json_schema::JsonSchema;
use governance::schema;
use governance::{init::init_state, Governance};
use helpers::encrypted_pass::EncryptedPass;
use helpers::network::*;
use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
use identity::keys::KeyPair;
use model::event::Event;
use model::request::*;
use model::signature::*;
use model::HashId;
use model::ValueWrapper;
use node::{Node, NodeMessage, NodeResponse, SubjectsTypes};
use request::{RequestHandler, RequestHandlerResponse};
use subject::{Subject, SubjectMessage, SubjectResponse};
use system::system;
use validation::{
    Validation, ValidationInfo, ValidationMessage
};


use lazy_static::lazy_static;

use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    /// The digest derivator for the system.
    pub static ref DIGEST_DERIVATOR: Mutex<DigestDerivator> = Mutex::new(DigestDerivator::Blake3_256);
    /// The key derivator for the system.
    pub static ref KEY_DERIVATOR: Mutex<KeyDerivator> = Mutex::new(KeyDerivator::Ed25519);

    pub static ref SCHEMAS: RwLock<HashMap<String, JsonSchema>> = {
        let mut schemas = HashMap::new();
        if let Ok(json_schema) = JsonSchema::compile(&schema()) {
            schemas.insert("governance".to_owned(), json_schema);
        };

        RwLock::new(schemas)
    };

    pub static ref CONTRACTS: RwLock<HashMap<String, Vec<u8>>> = {
        let contracts = HashMap::new();

        RwLock::new(contracts)
    };
}

pub struct Api {
    keys: KeyPair,
    request: ActorRef<RequestHandler>,
    node: ActorRef<Node>
}

impl Api {
    /// Creates a new `Api`.
    pub async fn new(keys: KeyPair, config: KoreBaseConfig, password: &str,) -> Result<Self, Error> {
        let system = match system(config, password).await {
            Ok(sys) => sys,
            Err(e) => todo!()
        };

        let node = Node::new(&keys).unwrap();
        let node_actor = match system.create_root_actor("node", node).await {
            Ok(actor) => actor,
            Err(e) => todo!()
        };


        let request = RequestHandler::new(keys.key_identifier());
        let request_actor =
            match system.create_root_actor("request", request).await {
                Ok(actor) => actor,
                Err(e) => todo!()
            };


        Ok(Self { keys, request: request_actor, node: node_actor })
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
