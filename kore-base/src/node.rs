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
    db::{Database, Storable},
    evaluation,
    helpers::encrypted_pass::EncryptedPass,
    model::{
        event::Ledger,
        request::EventRequest,
        signature::{Signature, Signed},
        HashId, SignTypesNode,
    },
    subject::{self, CreateSubjectData},
    validation::proof::ValidationProof,
    Api, Error, Event as KoreEvent, Subject, DIGEST_DERIVATOR,
};

use identity::{
    identifier::{
        derive::{digest::DigestDerivator, KeyDerivator},
        DigestIdentifier, KeyIdentifier,
    },
    keys::{
        Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair, Secp256k1KeyPair,
    },
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
    /// The node's known subjects in creation.
    creation_subjects: Vec<String>,
    /// The authorized governances.
    authorized_governances: Vec<String>,
}

impl Node {
    /// Creates a new node.
    pub fn new(id: &KeyPair) -> Result<Self, Error> {
        Ok(Self {
            owner: id.clone(),
            owned_subjects: Vec::new(),
            known_subjects: Vec::new(),
            authorized_governances: Vec::new(),
            creation_subjects: Vec::new(),
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
    pub fn add_authorized_governance(&mut self, governance_id: String) {
        self.authorized_governances.push(governance_id);
    }

    /// Gets the node's owned subjects.
    pub fn get_owned_subjects(&self) -> &Vec<String> {
        &self.owned_subjects
    }

    /// Gets the node's known subjects.
    pub fn get_known_subjects(&self) -> &Vec<String> {
        &self.known_subjects
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
}

/// Node message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeMessage {
    CreateNewSubjectLedger(Signed<Ledger>),
    CreateNewSubjectReq(CreateSubjectData),
    RegisterSubject(SubjectsTypes),
    SignRequest(SignTypesNode),
    AmISubjectOwner(String),
    IsAuthorized(String),
    GetOwnerIdentifier,
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
    IsAuthorized(bool),
    Error(Error),
    None,
}

impl Response for NodeResponse {}

/// Node event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeEvent {
    OwnedSubject(String),
    KnownSubject(String),
    AuthorizedSubject(String),
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
            NodeEvent::AuthorizedSubject(governance_id) => {
                self.add_authorized_governance(governance_id.clone());
            }
        }
    }

    /// Override the update method.
    fn update(&mut self, state: Self) {
        self.owned_subjects = state.owned_subjects;
        self.known_subjects = state.known_subjects;
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
            NodeMessage::CreateNewSubjectLedger(ledger) => {
                let subject = Subject::from_event(None, &ledger);
                let subject = match subject {
                    Ok(subject) => subject,
                    Err(e) => return Ok(NodeResponse::Error(e)),
                };

                // TODO cuando se crea un sujeto hay que guardar el evento de creación con la firma.
                if let Err(e) = ctx
                    .create_child(
                        &format!("{}", ledger.content.subject_id),
                        subject,
                    )
                    .await
                {
                    Ok(NodeResponse::Error(Error::Actor(format!("{}", e))))
                } else {
                    Ok(NodeResponse::SonWasCreated)
                }
            }
            NodeMessage::CreateNewSubjectReq(data) => {
                let subject = Subject::new(data.clone());

                if let Err(e) = ctx
                    .create_child(&format!("{}", data.subject_id), subject)
                    .await
                {
                    Ok(NodeResponse::Error(Error::Actor(format!("{}", e))))
                } else {
                    Ok(NodeResponse::SonWasCreated)
                }
            }
            NodeMessage::GetOwnerIdentifier => {
                Ok(NodeResponse::OwnerIdentifier(self.owner()))
            }
            NodeMessage::SignRequest(content) => {
                let sign = match content {
                    SignTypesNode::Validation(validation) => {
                        self.sign(&validation)
                    }
                    SignTypesNode::ValidationProofEvent(proof_event) => {
                        self.sign(&proof_event)
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
                    SignTypesNode::ApprovalReq(approval_req) => {
                        self.sign(&approval_req)
                    }
                    SignTypesNode::ApprovalRes(approval_res) => {
                        self.sign(&approval_res)
                    }
                    SignTypesNode::ApprovalSignature(approval_sign) => {
                        self.sign(&approval_sign)
                    }
                    SignTypesNode::Ledger(ledger) => self.sign(&ledger),
                    SignTypesNode::Event(event) => self.sign(&event),
                };

                match sign {
                    Ok(sign) => Ok(NodeResponse::SignRequest(sign)),
                    Err(e) => Ok(NodeResponse::Error(e)),
                }
            }
            NodeMessage::AmISubjectOwner(subject_id) => {
                Ok(NodeResponse::AmIOwner(
                    self.get_owned_subjects().iter().any(|x| **x == subject_id),
                ))
            }
            NodeMessage::RegisterSubject(subject) => {
                match subject {
                    SubjectsTypes::KnowSubject(subj) => {
                        ctx.publish_event(NodeEvent::KnownSubject(subj))
                            .await?;
                    }
                    SubjectsTypes::OwnerSubject(subj) => {
                        ctx.publish_event(NodeEvent::OwnedSubject(subj))
                            .await?;
                    }
                }
                Ok(NodeResponse::None)
            }
            NodeMessage::IsAuthorized(subject_id) => {
                // TODO Esto no se puede utilizar para governanza autorizada, tiene que ir a parte
                Ok(NodeResponse::IsAuthorized(
                    self.authorized_governances
                        .iter()
                        .any(|x| x.clone() == subject_id),
                ))
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
