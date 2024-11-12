// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Node module
//!

use std::path::Path;

use async_std::fs;
use nodekey::NodeKey;
use relationship::RelationShip;

use crate::{
    db::Storable,
    distribution::distributor::Distributor,
    helpers::db::LocalDB,
    model::{
        event::Ledger,
        signature::{Signature, Signed},
        HashId, SignTypesNode,
    },
    subject::CreateSubjectData,
    Error, Subject, SubjectMessage, SubjectResponse, DIGEST_DERIVATOR,
};

use identity::{
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    },
    keys::KeyPair,
};

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response, Sink,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

pub mod nodekey;
pub mod relationship;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct CompiledContract(Vec<u8>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SubjectsTypes {
    KnowSubject(String),
    OwnerSubject(String),
    TemporalSubject(String),
    ChangeTemp {
        subject_id: String,
        key_identifier: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubjectsVectors {
    pub owned_subjects: Vec<String>,
    /// The node's known subjects.
    pub known_subjects: Vec<String>,
    /// The authorized subjects.
    pub authorized_subjects: Vec<String>,

    pub temporal_subjects: Vec<String>,
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
    /// The authorized subjects.
    authorized_subjects: Vec<String>,

    temporal_subjects: Vec<String>,
}

impl Node {
    /// Creates a new node.
    pub fn new(id: &KeyPair) -> Result<Self, Error> {
        Ok(Self {
            owner: id.clone(),
            owned_subjects: Vec::new(),
            known_subjects: Vec::new(),
            authorized_subjects: Vec::new(),
            temporal_subjects: Vec::new(),
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

    /// Adds a subject to the node's temporal subjects.
    pub fn add_temporal_subject(&mut self, subject_id: String) {
        self.temporal_subjects.push(subject_id);
    }

    pub fn change_temporal_subject(
        &mut self,
        subject_id: String,
        key_identifier: String,
    ) {
        self.temporal_subjects.retain(|x| x.clone() != subject_id);
        if key_identifier == self.owner.key_identifier().to_string() {
            self.owned_subjects.push(subject_id);
        } else {
            self.known_subjects.push(subject_id);
        }
    }

    pub fn change_subject_owner(
        &mut self,
        subject_id: String,
        iam_owner: bool,
    ) {
        if iam_owner {
            self.known_subjects.retain(|x| x.clone() != subject_id);
            self.owned_subjects.push(subject_id);
        } else {
            self.owned_subjects.retain(|x| x.clone() != subject_id);
            self.known_subjects.push(subject_id);
        }
    }

    pub fn add_authorized_subject_id(&mut self, subject_id: String) {
        self.authorized_subjects.push(subject_id);
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
        Ok(())
    }

    async fn create_subjects(
        &self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        for subject in self.owned_subjects.clone() {
            ctx.create_child(&subject, Subject::default()).await?;
        }

        for subject in self.known_subjects.clone() {
            ctx.create_child(&subject, Subject::default()).await?;
        }

        for subject in self.temporal_subjects.clone() {
            ctx.create_child(&subject, Subject::default()).await?;
        }

        Ok(())
    }
}

/// Node message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeMessage {
    GetSubjects,
    CreateNewSubjectLedger(Signed<Ledger>),
    CreateNewSubjectReq(CreateSubjectData),
    RegisterSubject(SubjectsTypes),
    SignRequest(SignTypesNode),
    AmISubjectOwner(String),
    IsAuthorized(String),
    KnowSubject(String),
    ChangeSubjectOwner {
        subject_id: String,
        old_owner: String,
        new_owner: String,
    },
}

impl Message for NodeMessage {}

/// Node response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeResponse {
    Subjects(SubjectsVectors),
    RequestIdentifier(DigestIdentifier),
    SignRequest(Signature),
    SonWasCreated,
    OwnerIdentifier(KeyIdentifier),
    AmIOwner(bool),
    Contract(Vec<u8>),
    IsAuthorized(bool),
    KnowSubject(bool),
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
    TemporalSubject(String),
    ChangeTempSubj {
        subject_id: String,
        key_identifier: String,
    },
    ChangeSubjectOwner {
        iam_owner: bool,
        subject_id: String,
    },
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
        if let Err(_e) = Self::build_compilation_dir().await {
            // TODO manejar este error.
        };

        // Start store
        debug!("Creating Node store");
        self.init_store("node", None, false, ctx).await?;

        let node_key = NodeKey::new(self.owner());
        ctx.create_child("key", node_key).await?;

        self.create_subjects(ctx).await?;

        let distributor = Distributor {
            node: self.owner.key_identifier(),
        };
        ctx.create_child("distributor", distributor).await?;

        ctx.create_child("relation_ship", RelationShip::default())
            .await?;

        Ok(())
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
            NodeEvent::AuthorizedSubject(subject_id) => {
                self.add_authorized_subject_id(subject_id.clone());
            }
            NodeEvent::ChangeTempSubj {
                subject_id,
                key_identifier,
            } => {
                self.change_temporal_subject(
                    subject_id.clone(),
                    key_identifier.clone(),
                );
            }
            NodeEvent::TemporalSubject(subject_id) => {
                self.add_temporal_subject(subject_id.clone());
            }
            NodeEvent::ChangeSubjectOwner {
                iam_owner,
                subject_id,
            } => {
                self.change_subject_owner(subject_id.clone(), *iam_owner);
            }
        }
    }
}

// TODO: SI algo falla cuando un sujeto es temporal hay que eliminarlo y limpiar la base de datos.
#[async_trait]
impl Handler<Node> for Node {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: NodeMessage,
        ctx: &mut actor::ActorContext<Node>,
    ) -> Result<NodeResponse, ActorError> {
        match msg {
            NodeMessage::GetSubjects => {
                Ok(NodeResponse::Subjects(SubjectsVectors {
                    owned_subjects: self.owned_subjects.clone(),
                    known_subjects: self.known_subjects.clone(),
                    authorized_subjects: self.authorized_subjects.clone(),
                    temporal_subjects: self.temporal_subjects.clone(),
                }))
            }
            NodeMessage::ChangeSubjectOwner {
                subject_id,
                old_owner,
                new_owner,
            } => {
                if old_owner == self.owner.key_identifier().to_string() {
                    self.on_event(
                        NodeEvent::ChangeSubjectOwner {
                            iam_owner: false,
                            subject_id,
                        },
                        ctx,
                    )
                    .await;
                } else if new_owner == self.owner.key_identifier().to_string() {
                    self.on_event(
                        NodeEvent::ChangeSubjectOwner {
                            iam_owner: true,
                            subject_id,
                        },
                        ctx,
                    )
                    .await;
                }

                Ok(NodeResponse::None)
            }
            NodeMessage::CreateNewSubjectLedger(ledger) => {
                let Some(local_db): Option<LocalDB> =
                    ctx.system().get_helper("local_db").await
                else {
                    todo!()
                };

                let subject = Subject::from_event(None, &ledger);
                let subject = match subject {
                    Ok(subject) => subject,
                    Err(e) => return Ok(NodeResponse::Error(e)),
                };

                let subjec_actor = match ctx
                    .create_child(
                        &format!("{}", ledger.content.subject_id),
                        subject,
                    )
                    .await
                {
                    Ok(subject_actor) => {
                        let sink = Sink::new(
                            subject_actor.subscribe(),
                            local_db.get_subject(),
                        );
                        ctx.system().run_sink(sink).await;

                        self.on_event(
                            NodeEvent::TemporalSubject(
                                ledger.content.subject_id.to_string(),
                            ),
                            ctx,
                        )
                        .await;
                        subject_actor
                    }
                    Err(e) => {
                        return Ok(NodeResponse::Error(Error::Actor(format!(
                            "{}",
                            e
                        ))))
                    }
                };

                let response = match subjec_actor
                    .ask(SubjectMessage::UpdateLedger {
                        events: vec![ledger.clone()],
                    })
                    .await
                {
                    Ok(res) => res,
                    Err(_e) => todo!(),
                };

                match response {
                    SubjectResponse::Error(error) => todo!(),
                    SubjectResponse::LastSn(_) => {
                        self.on_event(
                            NodeEvent::ChangeTempSubj {
                                subject_id: ledger
                                    .content
                                    .subject_id
                                    .to_string(),
                                key_identifier: ledger
                                    .signature
                                    .signer
                                    .to_string(),
                            },
                            ctx,
                        )
                        .await;
                        Ok(NodeResponse::SonWasCreated)
                    }
                    _ => {
                        todo!()
                    }
                }
            }
            NodeMessage::CreateNewSubjectReq(data) => {
                let Some(local_db): Option<LocalDB> =
                    ctx.system().get_helper("local_db").await
                else {
                    todo!()
                };

                let subject = Subject::new(data.clone());

                match ctx
                    .create_child(&format!("{}", data.subject_id), subject)
                    .await
                {
                    Ok(actor) => {
                        let sink = Sink::new(
                            actor.subscribe(),
                            local_db.get_subject(),
                        );
                        ctx.system().run_sink(sink).await;

                        self.on_event(
                            NodeEvent::TemporalSubject(
                                data.subject_id.to_string(),
                            ),
                            ctx,
                        )
                        .await;
                        Ok(NodeResponse::SonWasCreated)
                    }
                    Err(e) => {
                        Ok(NodeResponse::Error(Error::Actor(format!("{}", e))))
                    }
                }
            }
            NodeMessage::SignRequest(content) => {
                let sign = match content {
                    SignTypesNode::EventRequest(event_req) => {
                        self.sign(&event_req)
                    }
                    SignTypesNode::Validation(validation) => {
                        self.sign(&*validation)
                    }
                    SignTypesNode::ValidationProofEvent(proof_event) => {
                        self.sign(&proof_event)
                    }
                    SignTypesNode::ValidationReq(validation_req) => {
                        self.sign(&*validation_req)
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
                        self.sign(&*approval_res)
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
                    self.owned_subjects.iter().any(|x| **x == subject_id),
                ))
            }
            NodeMessage::RegisterSubject(subject) => {
                match subject {
                    SubjectsTypes::KnowSubject(subj) => {
                        self.on_event(NodeEvent::KnownSubject(subj), ctx).await;
                    }
                    SubjectsTypes::OwnerSubject(subj) => {
                        self.on_event(NodeEvent::OwnedSubject(subj), ctx).await;
                    }
                    SubjectsTypes::TemporalSubject(subj) => {
                        self.on_event(NodeEvent::TemporalSubject(subj), ctx)
                            .await;
                    }
                    SubjectsTypes::ChangeTemp {
                        subject_id,
                        key_identifier,
                    } => {
                        self.on_event(
                            NodeEvent::ChangeTempSubj {
                                subject_id,
                                key_identifier,
                            },
                            ctx,
                        )
                        .await;
                    }
                }
                Ok(NodeResponse::None)
            }
            NodeMessage::IsAuthorized(subject_id) => {
                let auth_subj = self
                    .authorized_subjects
                    .iter()
                    .any(|x| x.clone() == subject_id);

                let owned_subj =
                    self.owned_subjects.iter().any(|x| x.clone() == subject_id);

                Ok(NodeResponse::IsAuthorized(auth_subj || owned_subj))
            }
            NodeMessage::KnowSubject(subject_id) => {
                let know_subj =
                    self.known_subjects.iter().any(|x| x.clone() == subject_id);

                let owned_subj =
                    self.owned_subjects.iter().any(|x| x.clone() == subject_id);

                Ok(NodeResponse::KnowSubject(know_subj || owned_subj))
            }
        }
    }

    async fn on_event(
        &mut self,
        event: NodeEvent,
        ctx: &mut ActorContext<Node>,
    ) {
        if let Err(_e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}

#[async_trait]
impl Storable for Node {}

#[cfg(test)]
pub mod tests {}
