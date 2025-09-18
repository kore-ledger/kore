// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Node module
//!

use std::{collections::HashMap, path::Path};

use nodekey::NodeKey;
use register::Register;
use relationship::RelationShip;
use tokio::fs;
use tracing::{error, warn};
use transfer::TransferRegister;

use crate::{
    DIGEST_DERIVATOR, Error, EventRequest, Subject, SubjectMessage,
    SubjectResponse,
    auth::{Auth, AuthMessage, AuthResponse},
    config::Config,
    db::Storable,
    distribution::distributor::Distributor,
    governance::Governance,
    helpers::db::ExternalDB,
    manual_distribution::ManualDistribution,
    model::{
        HashId, Namespace, SignTypesNode,
        event::{Ledger, LedgerValue},
        signature::{Signature, Signed},
    },
    subject::CreateSubjectData,
};

use identity::{
    identifier::{
        DigestIdentifier, KeyIdentifier, derive::digest::DigestDerivator,
    },
    keys::KeyPair,
};

use rush::{
    Actor, ActorContext, ActorPath, ChildAction, ActorError, Event,
    Handler, Message, Response, Sink,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use rush::{LightPersistence, PersistentActor};

pub mod nodekey;
pub mod register;
pub mod relationship;
pub mod transfer;

const TARGET_NODE: &str = "Kore-Node";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferSubject {
    pub name: String,
    pub subject_id: String,
    pub new_owner: String,
    pub actual_owner: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferData {
    pub name: String,
    pub new_owner: String,
    pub actual_owner: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubjectData {
    pub owner: String,
    pub governance_id: Option<String>,
    pub sn: u64,
    pub schema_id: String,
    pub namespace: Namespace,
}

/// Node struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    /// Owner of the node.
    owner: KeyPair,
    /// The node's owned subjects.
    owned_subjects: HashMap<String, SubjectData>,
    /// The node's known subjects.
    known_subjects: HashMap<String, SubjectData>,
    /// The node's temporal subjects.
    temporal_subjects: HashMap<String, SubjectData>,

    transfer_subjects: HashMap<String, TransferData>,
}

impl Node {
    /// Creates a new node.
    pub fn new(id: &KeyPair) -> Result<Self, Error> {
        Ok(Self {
            owner: id.clone(),
            owned_subjects: HashMap::new(),
            known_subjects: HashMap::new(),
            transfer_subjects: HashMap::new(),
            temporal_subjects: HashMap::new(),
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

    pub fn add_temporal_subject(
        &mut self,
        subject_id: String,
        data: SubjectData,
    ) {
        self.temporal_subjects.insert(subject_id, data);
    }

    /// Adds a subject to the node's owned subjects.
    pub fn transfer_subject(&mut self, data: TransferSubject) {
        self.transfer_subjects.insert(
            data.subject_id,
            TransferData {
                name: data.name,
                new_owner: data.new_owner,
                actual_owner: data.actual_owner,
            },
        );
    }

    pub fn delete_subject(&mut self, subject_id: &str) {
        self.temporal_subjects.remove(subject_id);
    }

    pub fn update_subject(&mut self, subject_id: String, sn: u64) {
        if let Some(mut data) = self.owned_subjects.get(&subject_id).cloned() {
            data.sn = sn;
            self.owned_subjects.insert(subject_id, data.clone());
        } else if let Some(mut data) =
            self.known_subjects.get(&subject_id).cloned()
        {
            data.sn = sn;
            self.known_subjects.insert(subject_id, data.clone());
        }
    }

    pub fn delete_transfer(&mut self, subject_id: String) {
        self.transfer_subjects.remove(&subject_id);
    }

    pub fn change_subject_owner(
        &mut self,
        subject_id: String,
        new_owner: Option<String>,
    ) {
        self.transfer_subjects.remove(&subject_id);

        if let Some(new_owner) = new_owner {
            if let Some(mut data) = self.owned_subjects.remove(&subject_id) {
                data.owner = new_owner;
                self.known_subjects.insert(subject_id, data);
            };
        } else if let Some(mut data) = self.known_subjects.remove(&subject_id) {
            data.owner = self.owner.key_identifier().to_string();
            self.owned_subjects.insert(subject_id, data);
        };
    }

    pub fn register_subject(&mut self, subject_id: String, iam_owner: bool) {
        if let Some(data) = self.temporal_subjects.remove(&subject_id) {
            if iam_owner {
                self.owned_subjects.insert(subject_id, data);
            } else {
                self.known_subjects.insert(subject_id, data);
            }
        };
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

        let our_key = self.owner.key_identifier();
        let our_key_string = our_key.to_string();

        for (subject, _) in self.owned_subjects.clone() {
            let subject_actor =
                ctx.create_child(&subject, Subject::default()).await?;
            let sink =
                Sink::new(subject_actor.subscribe(), ext_db.get_subject());
            ctx.system().run_sink(sink).await;

            ctx.create_child(
                &format!("distributor_{}", subject),
                Distributor {
                    node: our_key.clone(),
                },
            )
            .await?;
        }

        for (subject, data) in self.known_subjects.clone() {
            let i_new_owner =
                if let Some(transfer) = self.transfer_subjects.get(&subject) {
                    transfer.new_owner == our_key_string
                } else {
                    false
                };

            if data.governance_id.is_none() || i_new_owner {
                let subject_actor =
                    ctx.create_child(&subject, Subject::default()).await?;
                let sink =
                    Sink::new(subject_actor.subscribe(), ext_db.get_subject());
                ctx.system().run_sink(sink).await;
            }

            ctx.create_child(
                &format!("distributor_{}", subject),
                Distributor {
                    node: our_key.clone(),
                },
            )
            .await?;
        }

        Ok(())
    }
}

/// Node message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeMessage {
    // API
    SignRequest(SignTypesNode),
    PendingTransfers,
    // System actor
    UpDistributor(String),
    UpSubject(String, bool),
    GetSubjectData(String),
    UpdateSubject {
        subject_id: String,
        sn: u64,
    },
    RejectTransfer(String),
    TransferSubject(TransferSubject),
    DeleteSubject(String),
    CreateNewSubjectLedger(Signed<Ledger>),
    CreateNewSubjectReq(CreateSubjectData),
    OwnerPendingSubject(String),
    OldSubject(String),
    IsAuthorized(String),
    RegisterSubject {
        owner: String,
        subject_id: String,
    },
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
    SubjectData {
        data: SubjectData,
        new_owner: Option<String>,
    },
    PendingTransfers(Vec<TransferSubject>),
    RequestIdentifier(DigestIdentifier),
    SignRequest(Signature),
    SonWasCreated,
    OwnerIdentifier(KeyIdentifier),
    IOwnerPending((bool, bool)),
    IOld(bool),
    Contract(Vec<u8>),
    IsAuthorized {
        owned: bool,
        auth: bool,
        know: bool,
    },
    KnowSubject(bool),
    None,
}

impl Response for NodeResponse {}

/// Node event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeEvent {
    UpdateSubject {
        subject_id: String,
        sn: u64,
    },
    RejectTransfer(String),
    TemporalSubject {
        subject_id: String,
        data: SubjectData,
    },
    RegisterSubject {
        iam_owner: bool,
        subject_id: String,
    },
    ChangeSubjectOwner {
        new_owner: Option<String>,
        subject_id: String,
    },
    ConfirmTransfer(String),
    TransferSubject(TransferSubject),
    DeleteSubject(String),
}

impl Event for NodeEvent {}

#[async_trait]
impl Actor for Node {
    type Event = NodeEvent;
    type Message = NodeMessage;
    type Response = NodeResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut rush::ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Self::build_compilation_dir(ctx).await?;
        // Start store
        self.init_store("node", None, true, ctx).await?;

        ctx.create_child("register", Register::default()).await?;

        ctx.create_child("key", NodeKey::new(self.owner())).await?;

        ctx.create_child(
            "manual_distribution",
            ManualDistribution::new(self.owner()),
        )
        .await?;

        self.create_subjects(ctx).await?;

        ctx.create_child("auth", Auth::new(self.owner())).await?;

        ctx.create_child(
            "distributor",
            Distributor {
                node: self.owner.key_identifier(),
            },
        )
        .await?;
        ctx.create_child("relation_ship", RelationShip::default())
            .await?;

        ctx.create_child("transfer_register", TransferRegister::default())
            .await?;

        Ok(())
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl PersistentActor for Node {
    type Persistence = LightPersistence;

    /// Change node state.
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        match event {
            NodeEvent::UpdateSubject { subject_id, sn } => {
                self.update_subject(subject_id.clone(), *sn);
            }
            NodeEvent::ConfirmTransfer(subject_id) => {
                self.delete_transfer(subject_id.clone());
            }
            NodeEvent::RegisterSubject {
                iam_owner,
                subject_id,
            } => {
                self.register_subject(subject_id.clone(), *iam_owner);
            }
            NodeEvent::TemporalSubject { subject_id, data } => {
                self.add_temporal_subject(subject_id.clone(), data.clone());
            }
            NodeEvent::RejectTransfer(subject_id) => {
                self.delete_transfer(subject_id.clone());
            }
            NodeEvent::TransferSubject(transfer) => {
                self.transfer_subject(transfer.clone());
            }
            NodeEvent::ChangeSubjectOwner {
                new_owner,
                subject_id,
            } => {
                self.change_subject_owner(
                    subject_id.clone(),
                    new_owner.clone(),
                );
            }
            NodeEvent::DeleteSubject(subject_id) => {
                self.delete_subject(subject_id);
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
        ctx: &mut rush::ActorContext<Node>,
    ) -> Result<NodeResponse, ActorError> {
        match msg {
            NodeMessage::UpDistributor(subject_id) => {
                if let Err(e) = ctx
                    .create_child(
                        &format!("distributor_{}", subject_id),
                        Distributor {
                            node: self.owner.key_identifier(),
                        },
                    )
                    .await
                {
                    let e =
                        format!("Can not create distributor for subject {}", e);
                    error!("UpDistributor, {}", e);

                    ctx.system().stop_system();
                    let e = ActorError::FunctionalFail(e);
                    return Err(e);
                };

                Ok(NodeResponse::None)
            }
            NodeMessage::UpSubject(subject_id, light) => {
                let Some(ext_db): Option<ExternalDB> =
                    ctx.system().get_helper("ext_db").await
                else {
                    return Err(ActorError::NotHelper("ext_db".to_owned()));
                };

                let subject_actor =
                    ctx.create_child(&subject_id, Subject::default()).await?;
                if !light {
                    let sink = Sink::new(
                        subject_actor.subscribe(),
                        ext_db.get_subject(),
                    );
                    ctx.system().run_sink(sink).await;
                }

                Ok(NodeResponse::None)
            }
            NodeMessage::GetSubjectData(subject_id) => {
                let data = if let Some(data) =
                    self.owned_subjects.get(&subject_id)
                {
                    data.clone()
                } else if let Some(data) = self.known_subjects.get(&subject_id)
                {
                    data.clone()
                } else {
                    return Ok(NodeResponse::None);
                };

                let new_owner = self
                    .transfer_subjects
                    .get(&subject_id)
                    .map(|x| x.new_owner.clone());

                Ok(NodeResponse::SubjectData { data, new_owner })
            }
            NodeMessage::UpdateSubject { subject_id, sn } => {
                self.on_event(NodeEvent::UpdateSubject { subject_id, sn }, ctx)
                    .await;

                Ok(NodeResponse::None)
            }
            NodeMessage::PendingTransfers => {
                Ok(NodeResponse::PendingTransfers(
                    self.transfer_subjects
                        .iter()
                        .map(|x| TransferSubject {
                            name: x.1.name.clone(),
                            subject_id: x.0.clone(),
                            new_owner: x.1.new_owner.clone(),
                            actual_owner: x.1.actual_owner.clone(),
                        })
                        .collect(),
                ))
            }
            NodeMessage::RegisterSubject { owner, subject_id } => {
                let iam_owner =
                    owner == self.owner.key_identifier().to_string();
                self.on_event(
                    NodeEvent::RegisterSubject {
                        iam_owner,
                        subject_id,
                    },
                    ctx,
                )
                .await;

                Ok(NodeResponse::None)
            }
            NodeMessage::RejectTransfer(subject_id) => {
                self.on_event(NodeEvent::RejectTransfer(subject_id), ctx)
                    .await;
                Ok(NodeResponse::None)
            }
            NodeMessage::TransferSubject(data) => {
                self.on_event(NodeEvent::TransferSubject(data), ctx).await;
                Ok(NodeResponse::None)
            }
            NodeMessage::DeleteSubject(subject_id) => {
                self.on_event(NodeEvent::DeleteSubject(subject_id), ctx)
                    .await;

                Ok(NodeResponse::None)
            }
            NodeMessage::ChangeSubjectOwner {
                subject_id,
                old_owner,
                new_owner,
            } => {
                let our_key = self.owner.key_identifier().to_string();
                if old_owner == our_key {
                    self.on_event(
                        NodeEvent::ChangeSubjectOwner {
                            subject_id,
                            new_owner: Some(new_owner),
                        },
                        ctx,
                    )
                    .await;
                } else if new_owner == our_key {
                    self.on_event(
                        NodeEvent::ChangeSubjectOwner {
                            new_owner: None,
                            subject_id,
                        },
                        ctx,
                    )
                    .await;
                } else {
                    self.on_event(NodeEvent::ConfirmTransfer(subject_id), ctx)
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
                    ctx.system().stop_system();
                    return Err(ActorError::NotHelper("ext_db".to_owned()));
                };

                let subject = if let EventRequest::Create(create_event) =
                    ledger.content.event_request.content.clone()
                {
                    let properties = if create_event.schema_id == "governance" {
                        let gov = Governance::new(
                            ledger
                                .content
                                .event_request
                                .signature
                                .signer
                                .clone(),
                        );
                        gov.to_value_wrapper().map_err(|e| {
                            error!(
                                TARGET_NODE,
                                "CreateNewSubjectLedger, {}", e
                            );
                            ActorError::FunctionalFail(e.to_string())
                        })?
                    } else if let LedgerValue::Patch(init_state) =
                        ledger.content.value.clone()
                    {
                        init_state
                    } else {
                        let e = "Can not create subject, ledgerValue is not a patch";
                        warn!(TARGET_NODE, "CreateNewSubjectLedger, {}", e);
                        return Err(ActorError::Functional(e.to_string()));
                    };

                    Subject::from_event(&ledger, properties)
                        .map_err(|e| {
                            warn!(TARGET_NODE, "CreateNewSubjectLedger, Can not create subject from event {}", e);
                            ActorError::Functional(e.to_string())
                        })?
                } else {
                    let e = "trying to create a subject without create event";
                    warn!(TARGET_NODE, "CreateNewSubjectLedger, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                };

                let governance_id = if subject.governance_id.is_empty() {
                    None
                } else {
                    Some(subject.governance_id.to_string())
                };

                self.on_event(
                    NodeEvent::TemporalSubject {
                        subject_id: subject.subject_id.to_string(),
                        data: SubjectData {
                            owner: subject.creator.to_string(),
                            governance_id,
                            sn: 0,
                            schema_id: subject.schema_id.clone(),
                            namespace: subject.namespace.clone(),
                        },
                    },
                    ctx,
                )
                .await;

                let subject_actor = ctx
                    .create_child(
                        &format!("{}", ledger.content.subject_id),
                        subject.clone(),
                    )
                    .await
                    .map_err(|e| ActorError::Functional(e.to_string()))?;

                let sink =
                    Sink::new(subject_actor.subscribe(), ext_db.get_subject());
                ctx.system().run_sink(sink).await;

                let response = subject_actor
                    .ask(SubjectMessage::UpdateLedger {
                        events: vec![ledger.clone()],
                    })
                    .await?;

                self.on_event(
                    NodeEvent::RegisterSubject {
                        iam_owner: self.owner.key_identifier() == subject.owner,
                        subject_id: subject.subject_id.to_string(),
                    },
                    ctx,
                )
                .await;

                ctx.create_child(
                    &format!("distributor_{}", ledger.content.subject_id),
                    Distributor { node: self.owner() },
                )
                .await
                .map_err(|e| ActorError::Functional(e.to_string()))?;

                match response {
                    SubjectResponse::UpdateResult(_, _, _) => {
                        Ok(NodeResponse::SonWasCreated)
                    }
                    _ => {
                        ctx.system().stop_system();
                        let e = ActorError::UnexpectedResponse(
                            subject_actor.path(),
                            "SubjectResponse::UpdateResult".to_owned(),
                        );
                        return Err(e);
                    }
                }
            }
            NodeMessage::CreateNewSubjectReq(data) => {
                let Some(ext_db): Option<ExternalDB> =
                    ctx.system().get_helper("ext_db").await
                else {
                    ctx.system().stop_system();
                    error!(
                        TARGET_NODE,
                        "CreateNewSubjectReq, Can not obtain ext_db helper"
                    );
                    return Err(ActorError::NotHelper("ext_db".to_owned()));
                };

                let subject = Subject::new(data.clone());

                let child = ctx
                    .create_child(&format!("{}", data.subject_id), subject)
                    .await?;

                let sink = Sink::new(child.subscribe(), ext_db.get_subject());
                ctx.system().run_sink(sink).await;

                let governance_id = if data.create_req.governance_id.is_empty()
                {
                    None
                } else {
                    Some(data.create_req.governance_id.to_string())
                };

                self.on_event(
                    NodeEvent::TemporalSubject {
                        subject_id: data.subject_id.to_string(),
                        data: SubjectData {
                            owner: data.creator.to_string(),
                            governance_id,
                            sn: 0,
                            schema_id: data.create_req.schema_id,
                            namespace: data.create_req.namespace,
                        },
                    },
                    ctx,
                )
                .await;

                ctx.create_child(
                    &format!("distributor_{}", data.subject_id),
                    Distributor { node: self.owner() },
                )
                .await?;

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
            NodeMessage::OwnerPendingSubject(subject_id) => {
                let our_key = self.owner.key_identifier().to_string();

                Ok(NodeResponse::IOwnerPending((
                    self.owned_subjects.keys().any(|x| **x == subject_id),
                    if let Some(data) = self.transfer_subjects.get(&subject_id)
                    {
                        data.new_owner == our_key
                    } else {
                        false
                    },
                )))
            }
            NodeMessage::OldSubject(subject_id) => {
                let our_key = self.owner.key_identifier().to_string();

                Ok(NodeResponse::IOld(
                    if let Some(data) = self.transfer_subjects.get(&subject_id)
                    {
                        data.actual_owner == our_key
                    } else {
                        false
                    },
                ))
            }
            NodeMessage::IsAuthorized(subject_id) => {
                let auth: Option<rush::ActorRef<Auth>> =
                    ctx.get_child("auth").await;
                let authorized_subjects = if let Some(auth) = auth {
                    let res = match auth.ask(AuthMessage::GetAuths).await {
                        Ok(res) => res,
                        Err(e) => {
                            ctx.system().stop_system();
                            return Err(e);
                        }
                    };
                    let AuthResponse::Auths { subjects } = res else {
                        ctx.system().stop_system();
                        let e = ActorError::UnexpectedResponse(
                            ActorPath::from(format!("{}/auth", ctx.path())),
                            "AuthResponse::Auths".to_owned(),
                        );
                        return Err(e);
                    };
                    subjects
                } else {
                    ctx.system().stop_system();
                    let e = ActorError::NotFound(ActorPath::from(format!(
                        "{}/auth",
                        ctx.path()
                    )));
                    return Err(e);
                };

                let auth_subj =
                    authorized_subjects.iter().any(|x| x.clone() == subject_id);

                let owned_subj =
                    self.owned_subjects.keys().any(|x| x.clone() == subject_id);

                let know_subj =
                    self.known_subjects.keys().any(|x| x.clone() == subject_id);

                Ok(NodeResponse::IsAuthorized {
                    auth: auth_subj,
                    owned: owned_subj,
                    know: know_subj,
                })
            }
        }
    }

    async fn on_child_fault(
        &mut self,
        _error: ActorError,
        ctx: &mut ActorContext<Node>,
    ) -> ChildAction {
        ctx.system().stop_system();
        ChildAction::Stop
    }

    async fn on_event(
        &mut self,
        event: NodeEvent,
        ctx: &mut ActorContext<Node>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            error!(TARGET_NODE, "OnEvent, can not persist information: {}", e);
            ctx.system().stop_system();
        };
    }
}

#[async_trait]
impl Storable for Node {}

#[cfg(test)]
pub mod tests {}
