// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Node module
//!

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use async_std::fs;

use crate::{
    db::{Database, Storable}, evaluation, helpers::encrypted_pass::EncryptedPass, model::{
        event::Ledger, request::EventRequest, signature::{Signature, Signed}, HashId, SignTypesNode
    }, subject, validation::proof::ValidationProof, Api, Error, Event as KoreEvent, Subject, DIGEST_DERIVATOR
};

use identity::{
    identifier::{
        derive::{digest::DigestDerivator, KeyDerivator}, DigestIdentifier, KeyIdentifier,
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair, Secp256k1KeyPair},
};

use actor::{
    Actor, ActorContext, ActorPath, ActorSystem, Error as ActorError, Event,
    Handler, Message, Response, SystemRef,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct CompiledContract(Vec<u8>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SubjectsTypes {
    KnowSubject(String),
    OwnerSubject(String),
    KnowGovernance(String),
    OwnerGovernance(String),
}

/// Node struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    /// Owner of the node.
    #[serde(skip)]
    owner: KeyPair,
    /// The node's owned subjects.
    owned_subjects: Vec<String>,
    /// The node's known subjects.
    known_subjects: Vec<String>,
    /// The node's owned governances.
    owned_governances: Vec<String>,
    /// The node's known governances.
    known_governances: Vec<String>,
    /// Compiled contracts
    compiled_contracts: HashMap<String, CompiledContract>,
}

impl Node {
    /// Creates a new node.
    pub fn new(id: &KeyPair) -> Result<Self, Error> {
        Ok(Self {
            owner: id.clone(),
            owned_subjects: Vec::new(),
            known_subjects: Vec::new(),
            owned_governances: Vec::new(),
            known_governances: Vec::new(),
            compiled_contracts: HashMap::new(),
        })
    }

    /// Gets the node's owner identifier.
    ///
    /// # Returns
    ///
    /// A `KeyIdentifier` with the node's owner identifier.
    ///
    pub fn owner(&self) -> KeyIdentifier {
        self.owner.key_identifier()
    }

    /// Adds a subject to the node's known subjects.
    pub fn add_known_subject(&mut self, subject_id: String) {
        self.known_subjects.push(subject_id);
    }

    /// Adds a subject to the node's owned subjects.
    pub fn add_owned_subject(&mut self, subject_id: String) {
        self.owned_subjects.push(subject_id);
    }

    /// Adds a governance to the node's known governances.
    pub fn add_known_governance(&mut self, governance_id: String) {
        self.known_governances.push(governance_id);
    }

    /// Adds a governance to the node's owned governances.
    pub fn add_owned_governance(&mut self, governance_id: String) {
        self.owned_governances.push(governance_id);
    }

    /// Gets the node's owned subjects.
    pub fn get_owned_subjects(&self) -> &Vec<String> {
        &self.owned_subjects
    }

    /// Gets the node's known subjects.
    pub fn get_known_subjects(&self) -> &Vec<String> {
        &self.known_subjects
    }

    /// Gets the node's owned governances.
    pub fn get_owned_governances(&self) -> &Vec<String> {
        &self.owned_governances
    }

    /// Gets the node's known governances.
    pub fn get_known_governances(&self) -> &Vec<String> {
        &self.known_governances
    }

    fn sign<T: HashId>(&self, content: &T) -> Result<Signature, Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        Signature::new(content, &self.owner, derivator)
            .map_err(|e| Error::Signature(format!("{}", e)))
    }

    async fn build_compilation_dir() -> Result<(), Error> {
        if !Path::new("contracts").exists() {
            fs::create_dir("contracts").await.map_err(|e| {
                Error::Node(format!("Can not create contracts dir: {}", e))
            })?;
        }

        let toml: String = Self::compilation_toml();
        // We write cargo.toml
        fs::write("contracts/Cargo.toml", toml).await.map_err(|e| {
            Error::Node(format!("Can not create Cargo.toml file: {}", e))
        })?;

        if !Path::new("contracts/src/bin").exists() {
            fs::create_dir_all("contracts/src/bin").await.map_err(|e| {
                Error::Node(format!("Can not create src dir: {}", e))
            })?;
        }
        Ok(())
    }

    fn compilation_toml() -> String {
        r#"
    [package]
    name = "contract"
    version = "0.1.0"
    edition = "2021"
    
    [dependencies]
    serde = { version = "1.0.208", features = ["derive"] }
    serde_json = "1.0.125"
    json-patch = "2.0.0"
    thiserror = "1.0.63"
    kore-contract-sdk = { git = "https://github.com/kore-ledger/kore-contract-sdk.git", branch = "main"}
    
    [profile.release]
    strip = "debuginfo"
    lto = true
    
    [lib]
    crate-type = ["cdylib"]
      "#
        .into()
    }

    pub fn get_contract(&self, contract_path: &str) -> Result<Vec<u8>, Error> {
        if let Some(contract) = self.compiled_contracts.get(contract_path) {
            Ok(contract.0.clone())
        } else {
            Err(Error::Governance(format!(
                "can not find this schema {}",
                contract_path
            )))
        }
    }
}

/// Node message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeMessage {
    CreateNewSubject(Ledger, KeyPair),
    RegisterSubject(SubjectsTypes),
    SignRequest(SignTypesNode),
    AmISubjectOwner(DigestIdentifier),
    AmIGovernanceOwner(DigestIdentifier),
    IKnowThisGov(String),
    GetOwnerIdentifier,
    CompiledContract(String),
}

impl Message for NodeMessage {}

/// Node response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeResponse {
    /// Event request.
    RequestIdentifier(DigestIdentifier),
    SignRequest(Signature),
    SonWasCreated,
    /// Owner identifier.
    OwnerIdentifier(KeyIdentifier),
    AmIOwner(bool),
    Contract(Vec<u8>),
    IKnowThisGov(bool),
    Error(Error),
    None,
}

impl Response for NodeResponse {}

/// Node event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeEvent {
    OwnedSubject(String),
    KnownSubject(String),
    OwnedGovernance(String),
    KnownGovernance(String),
}

impl Event for NodeEvent {}

#[async_trait]
impl Actor for Node {
    type Event = NodeEvent;
    type Message = NodeMessage;
    type Response = NodeResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut actor::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        if let Err(e) = Self::build_compilation_dir().await {
            // TODO manejar este error.
        };
        // Start store
        debug!("Creating Node store");
        self.init_store("node", None, false, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Stopping Node store");
        self.stop_store(ctx).await?;
        Ok(())
    }
}

#[async_trait]
impl PersistentActor for Node {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            NodeEvent::OwnedSubject(subject_id) => {
                self.add_owned_subject(subject_id.clone());
            }
            NodeEvent::KnownSubject(subject_id) => {
                self.add_known_subject(subject_id.clone());
            }
            NodeEvent::OwnedGovernance(governance_id) => {
                self.add_owned_governance(governance_id.clone());
            }
            NodeEvent::KnownGovernance(governance_id) => {
                self.add_known_governance(governance_id.clone());
            }
        }
    }

    /// Override the update method.
    fn update(&mut self, state: Self) {
        self.owned_subjects = state.owned_subjects;
        self.known_subjects = state.known_subjects;
        self.owned_governances = state.owned_governances;
        self.known_governances = state.known_governances;
    }
}

#[async_trait]
impl Handler<Node> for Node {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: NodeMessage,
        ctx: &mut actor::ActorContext<Node>,
    ) -> Result<NodeResponse, ActorError> {
        match msg {
            NodeMessage::CreateNewSubject(ledger, subject_keys) => {
                let subject = Subject::from_event(subject_keys, &ledger);
                let subject = match subject {
                    Ok(subject) => subject,
                    Err(e) => return Ok(NodeResponse::Error(e)),
                };

                if let Err(e) = ctx.create_child(&format!("{}", ledger.subject_id), subject).await {
                    Ok(NodeResponse::Error(Error::Actor(format!("{}", e))))
                } else {
                    Ok(NodeResponse::SonWasCreated)    
                }
            },
            NodeMessage::GetOwnerIdentifier => {
                Ok(NodeResponse::OwnerIdentifier(self.owner()))
            }
            NodeMessage::SignRequest(content) => {
                let sign = match content {
                    SignTypesNode::Validation(validation) => {
                        self.sign(&validation)
                    }
                    SignTypesNode::ValidationReq(validation_req) => {
                        self.sign(&validation_req)
                    }
                    SignTypesNode::ValidationRes(validation_res) => {
                        self.sign(&validation_res)
                    }
                    SignTypesNode::EvaluationReq(evaluation_req) => {
                        self.sign(&evaluation_req)
                    }
                    SignTypesNode::EvaluationRes(evaluation_res) => {
                        self.sign(&evaluation_res)
                    }
                };

                match sign {
                    Ok(sign) => Ok(NodeResponse::SignRequest(sign)),
                    Err(e) => Ok(NodeResponse::Error(e)),
                }
            }
            NodeMessage::AmISubjectOwner(subject_id) => {
                Ok(NodeResponse::AmIOwner(
                    self.get_owned_subjects()
                        .iter()
                        .any(|x| **x == subject_id.to_string()),
                ))
            }
            NodeMessage::AmIGovernanceOwner(governance_id) => {
                Ok(NodeResponse::AmIOwner(
                    self.get_owned_governances()
                        .iter()
                        .any(|x| **x == governance_id.to_string()),
                ))
            }
            NodeMessage::RegisterSubject(subject) => {
                match subject {
                    SubjectsTypes::KnowSubject(subj) => {
                        ctx.event(NodeEvent::KnownSubject(subj)).await?;
                    }
                    SubjectsTypes::OwnerSubject(subj) => {
                        ctx.event(NodeEvent::OwnedSubject(subj)).await?;
                    }
                    SubjectsTypes::KnowGovernance(gov) => {
                        ctx.event(NodeEvent::KnownGovernance(gov)).await?;
                    }
                    SubjectsTypes::OwnerGovernance(gov) => {
                        ctx.event(NodeEvent::OwnedGovernance(gov)).await?;
                    }
                }
                Ok(NodeResponse::None)
            }
            NodeMessage::CompiledContract(contract_path) => {
                match self.get_contract(&contract_path) {
                    Ok(contract) => Ok(NodeResponse::Contract(contract)),
                    Err(e) => Ok(NodeResponse::Error(e)),
                }
            }
            NodeMessage::IKnowThisGov(subject_id) => {
                let know_gov = self.known_governances.iter().any(|x| x.clone() == subject_id);
                let our_gov = self.owned_governances.iter().any(|x| x.clone() == subject_id);

                Ok(NodeResponse::IKnowThisGov(know_gov || our_gov))
            }
        }
    }

    async fn on_event(
        &mut self,
        event: NodeEvent,
        ctx: &mut ActorContext<Node>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}

#[async_trait]
impl Storable for Node {}

#[cfg(test)]
pub mod tests {

    use super::*;
}
