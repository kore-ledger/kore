// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Node module
//!

use std::path::Path;

use async_std::fs;
use nodekey::NodeKey;
use register::Register;
use relationship::RelationShip;
use tracing::{error, warn};

use crate::{
    auth::{Auth, AuthMessage, AuthResponse}, config::Config, db::Storable, distribution::distributor::Distributor, governance::init::init_state, helpers::{db::ExternalDB, sink::KoreSink}, manual_distribution::ManualDistribution, model::{
        event::{Ledger, LedgerValue},
        signature::{Signature, Signed},
        HashId, SignTypesNode,
    }, subject::CreateSubjectData, Error, EventRequest, Subject, SubjectMessage, SubjectResponse, DIGEST_DERIVATOR
};

use identity::{
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    },
    keys::KeyPair,
};

use actor::{
    Actor, ActorContext, ActorPath, ChildAction, Error as ActorError, Event,
    Handler, Message, Response, Sink, SystemEvent,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;

pub mod nodekey;
pub mod register;
pub mod relationship;

const TARGET_NODE: &str = "Kore-Node";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct CompiledContract(Vec<u8>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SubjectsTypes {
    KnowSubject(String),
    OwnerSubject(String),
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
}

/// Node struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    /// Owner of the node.
    owner: KeyPair,
    /// The node's owned subjects.
    owned_subjects: Vec<String>,
    /// The node's known subjects.
    known_subjects: Vec<String>,
}

impl Node {
    /// Creates a new node.
    pub fn new(id: &KeyPair) -> Result<Self, Error> {
        Ok(Self {
            owner: id.clone(),
            owned_subjects: Vec::new(),
            known_subjects: Vec::new(),
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

    pub fn delete_subject(&mut self, subject_id: String, iam_owner: bool) {
        if iam_owner {
            self.owned_subjects.retain(|x| x.clone() != subject_id);
        } else {
            self.known_subjects.retain(|x| x.clone() != subject_id);
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

    fn sign<T: HashId>(&self, content: &T) -> Result<Signature, Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!(TARGET_NODE, "Error getting derivator");
            DigestDerivator::Blake3_256
        };
        Signature::new(content, &self.owner, derivator)
            .map_err(|e| Error::Signature(format!("{}", e)))
    }

    async fn build_compilation_dir(
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        let Some(config): Option<Config> =
            ctx.system().get_helper("config").await
        else {
            return Err(ActorError::NotHelper("config".to_owned()));
        };

        let dir = format!("{}/contracts", config.contracts_dir);

        if !Path::new(&dir).exists() {
            fs::create_dir_all(&dir).await.map_err(|e| {
                ActorError::FunctionalFail(format!(
                    "Can not create contracts dir: {}",
                    e
                ))
            })?;
        }
        Ok(())
    }

    async fn create_subjects(
        &self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        let Some(ext_db): Option<ExternalDB> =
            ctx.system().get_helper("ext_db").await
        else {
            return Err(ActorError::NotHelper("ext_db".to_owned()));
        };

        let Some(kore_sink): Option<KoreSink> =
            ctx.system().get_helper("sink").await
        else {
            return Err(ActorError::NotHelper("sink".to_owned()));
        };

        for subject in self.owned_subjects.clone() {
            let subject_actor =
                ctx.create_child(&subject, Subject::default()).await?;
            let sink =
                Sink::new(subject_actor.subscribe(), ext_db.get_subject());
            ctx.system().run_sink(sink).await;

            let sink = Sink::new(subject_actor.subscribe(), kore_sink.clone());
            ctx.system().run_sink(sink).await;
        }

        for subject in self.known_subjects.clone() {
            let subject_actor =
                ctx.create_child(&subject, Subject::default()).await?;
            let sink =
                Sink::new(subject_actor.subscribe(), ext_db.get_subject());
            ctx.system().run_sink(sink).await;

            let sink = Sink::new(subject_actor.subscribe(), kore_sink.clone());
            ctx.system().run_sink(sink).await;
        }

        Ok(())
    }
}

/// Node message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeMessage {
    DeleteSubject {
        owner: String,
        subject_id: String,
    },
    GetSubjects,
    CreateNewSubjectLedger(Signed<Ledger>),
    CreateNewSubjectReq(CreateSubjectData),
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
    None,
}

impl Response for NodeResponse {}

/// Node event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeEvent {
    OwnedSubject(String),
    KnownSubject(String),
    ChangeSubjectOwner { iam_owner: bool, subject_id: String },
    DeleteSubject { iam_owner: bool, subject_id: String },
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
        Self::build_compilation_dir(ctx).await?;
        // Start store
        self.init_store("node", None, true, ctx).await?;

        let register = Register::default();
        ctx.create_child("register", register).await?;

        let node_key = NodeKey::new(self.owner());
        ctx.create_child("key", node_key).await?;

        let manual_dis = ManualDistribution::new(self.owner());
        ctx.create_child("manual_dis", manual_dis).await?;

        self.create_subjects(ctx).await?;

        let distributor = Distributor {
            node: self.owner.key_identifier(),
        };

        let auth = Auth::new(self.owner());
        ctx.create_child("auth", auth).await?;

        ctx.create_child("distributor", distributor).await?;
        ctx.create_child("relation_ship", RelationShip::default())
            .await?;

        Ok(())
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await?;
        Ok(())
    }
}

#[async_trait]
impl PersistentActor for Node {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        match event {
            NodeEvent::OwnedSubject(subject_id) => {
                self.add_owned_subject(subject_id.clone());
            }
            NodeEvent::KnownSubject(subject_id) => {
                self.add_known_subject(subject_id.clone());
            }
            NodeEvent::ChangeSubjectOwner {
                iam_owner,
                subject_id,
            } => {
                self.change_subject_owner(subject_id.clone(), *iam_owner);
            }
            NodeEvent::DeleteSubject {
                iam_owner,
                subject_id,
            } => {
                self.delete_subject(subject_id.clone(), *iam_owner);
            }
        };

        Ok(())
    }
}

#[async_trait]
impl Handler<Node> for Node {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: NodeMessage,
        ctx: &mut actor::ActorContext<Node>,
    ) -> Result<NodeResponse, ActorError> {
        match msg {
            NodeMessage::DeleteSubject { owner, subject_id } => {
                if owner == self.owner.key_identifier().to_string() {
                    self.on_event(
                        NodeEvent::DeleteSubject {
                            iam_owner: true,
                            subject_id,
                        },
                        ctx,
                    )
                    .await;
                } else {
                    self.on_event(
                        NodeEvent::DeleteSubject {
                            iam_owner: false,
                            subject_id,
                        },
                        ctx,
                    )
                    .await;
                }

                Ok(NodeResponse::None)
            }
            NodeMessage::GetSubjects => {
                Ok(NodeResponse::Subjects(SubjectsVectors {
                    owned_subjects: self.owned_subjects.clone(),
                    known_subjects: self.known_subjects.clone(),
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
                let Some(ext_db): Option<ExternalDB> =
                    ctx.system().get_helper("ext_db").await
                else {
                    error!(
                        TARGET_NODE,
                        "CreateNewSubjectLedger, Can not obtain ext_db helper"
                    );
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper("ext_db".to_owned()));
                };

                let Some(kore_sink): Option<KoreSink> =
                    ctx.system().get_helper("sink").await
                else {
                    error!(
                        TARGET_NODE,
                        "CreateNewSubjectLedger, Can not obtain sink helper"
                    );
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    return Err(ActorError::NotHelper("sink".to_owned()));
                };

                let subject = if let EventRequest::Create(create_event) =
                    ledger.content.event_request.content.clone()
                {
                    let properties = if create_event.schema_id == "governance" {
                        init_state(
                            &ledger
                                .content
                                .event_request
                                .signature
                                .signer
                                .to_string(),
                        )
                    } else if let LedgerValue::Patch(init_state) =
                        ledger.content.value.clone()
                    {
                        init_state
                    } else {
                        let e = "Can not create subject, ledgerValue is not a patch";
                        warn!(TARGET_NODE, "CreateNewSubjectLedger, {}", e);
                        return Err(ActorError::Functional(e.to_string()));
                    };

                    Subject::from_event(None, &ledger, properties)
                        .map_err(|e| {
                            warn!(TARGET_NODE, "CreateNewSubjectLedger, Can not create subject from event {}", e);
                            ActorError::Functional(e.to_string())
                        })?
                } else {
                    let e = "trying to create a subject without create event";
                    warn!(TARGET_NODE, "CreateNewSubjectLedger, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                };

                let subject_actor = ctx
                    .create_child(
                        &format!("{}", ledger.content.subject_id),
                        subject,
                    )
                    .await?;

                let sink =
                    Sink::new(subject_actor.subscribe(), ext_db.get_subject());
                ctx.system().run_sink(sink).await;

                let sink =
                    Sink::new(subject_actor.subscribe(), kore_sink.clone());
                ctx.system().run_sink(sink).await;

                let response = subject_actor
                    .ask(SubjectMessage::UpdateLedger {
                        events: vec![ledger.clone()],
                    })
                    .await?;

                match response {
                    SubjectResponse::LastSn(_) => {
                        let event = if ledger.signature.signer
                            == self.owner.key_identifier()
                        {
                            NodeEvent::OwnedSubject(
                                ledger.content.subject_id.to_string(),
                            )
                        } else {
                            NodeEvent::KnownSubject(
                                ledger.content.subject_id.to_string(),
                            )
                        };

                        self.on_event(event, ctx).await;
                        Ok(NodeResponse::SonWasCreated)
                    }
                    _ => {
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        let e = ActorError::UnexpectedResponse(
                            subject_actor.path(),
                            "SubjectResponse::LastSn".to_owned(),
                        );
                        return Err(e);
                    }
                }
            }
            NodeMessage::CreateNewSubjectReq(data) => {
                let Some(ext_db): Option<ExternalDB> =
                    ctx.system().get_helper("ext_db").await
                else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    error!(
                        TARGET_NODE,
                        "CreateNewSubjectReq, Can not obtain ext_db helper"
                    );
                    return Err(ActorError::NotHelper("ext_db".to_owned()));
                };

                let Some(kore_sink): Option<KoreSink> =
                    ctx.system().get_helper("sink").await
                else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    error!(
                        TARGET_NODE,
                        "CreateNewSubjectReq, Can not obtain sink helper"
                    );
                    return Err(ActorError::NotHelper("sink".to_owned()));
                };

                let subject = Subject::new(data.clone());

                let child = ctx
                    .create_child(&format!("{}", data.subject_id), subject)
                    .await?;

                let sink = Sink::new(child.subscribe(), ext_db.get_subject());
                ctx.system().run_sink(sink).await;

                let sink = Sink::new(child.subscribe(), kore_sink.clone());
                ctx.system().run_sink(sink).await;

                self.on_event(
                    NodeEvent::OwnedSubject(data.subject_id.to_string()),
                    ctx,
                )
                .await;

                Ok(NodeResponse::SonWasCreated)
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
                }
                .map_err(|e| {
                    warn!(
                        TARGET_NODE,
                        "SignRequest, Can not sign event: {}", e
                    );
                    ActorError::FunctionalFail(format!(
                        "Can not sign event: {}",
                        e
                    ))
                })?;

                Ok(NodeResponse::SignRequest(sign))
            }
            NodeMessage::AmISubjectOwner(subject_id) => {
                Ok(NodeResponse::AmIOwner(
                    self.owned_subjects.iter().any(|x| **x == subject_id),
                ))
            }
            NodeMessage::IsAuthorized(subject_id) => {
                let auth: Option<actor::ActorRef<Auth>> =
                    ctx.get_child("auth").await;
                let authorized_subjects = if let Some(auth) = auth {
                    let res = match auth.ask(AuthMessage::GetAuths).await {
                        Ok(res) => res,
                        Err(e) => {
                            ctx.system()
                                .send_event(SystemEvent::StopSystem)
                                .await;
                            return Err(e);
                        }
                    };
                    let AuthResponse::Auths { subjects } = res else {
                        ctx.system().send_event(SystemEvent::StopSystem).await;
                        let e = ActorError::UnexpectedResponse(
                            ActorPath::from(format!("{}/auth", ctx.path())),
                            "AuthResponse::Auths".to_owned(),
                        );
                        return Err(e);
                    };
                    subjects
                } else {
                    ctx.system().send_event(SystemEvent::StopSystem).await;
                    let e = ActorError::NotFound(ActorPath::from(format!(
                        "{}/auth",
                        ctx.path()
                    )));
                    return Err(e);
                };

                let auth_subj =
                    authorized_subjects.iter().any(|x| x.clone() == subject_id);

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

    async fn on_child_fault(
        &mut self,
        _error: ActorError,
        ctx: &mut ActorContext<Node>,
    ) -> ChildAction {
        ctx.system().send_event(SystemEvent::StopSystem).await;
        ChildAction::Stop
    }

    async fn on_event(
        &mut self,
        event: NodeEvent,
        ctx: &mut ActorContext<Node>,
    ) {
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(TARGET_NODE, "OnEvent, can not persist information: {}", e);
            ctx.system().send_event(SystemEvent::StopSystem).await;
        };
    }
}

#[async_trait]
impl Storable for Node {}

#[cfg(test)]
pub mod tests {}
