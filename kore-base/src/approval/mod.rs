// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{
    Actor, ActorContext, Error as ActorError, Handler, Message, Response,
};
use actor::{ActorPath, ActorRef, Event};
use approver::{Approver, ApproverCommand};
use async_trait::async_trait;
use identity::identifier::derive::digest::DigestDerivator;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use memsecurity::Blake3Hash;
use request::ApprovalReq;
use response::ApprovalRes;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error, event};

use crate::evaluation::response::EvaluationRes;
use crate::governance::model::Roles;
use crate::governance::RequestStage;
use crate::model::{namespace, Namespace, SignTypesNode};
use crate::subject::event::{
    LedgerEvent, LedgerEventCommand, LedgerEventResponse,
};
use crate::validation::response::ValidationRes;
use crate::{
    db::Storable,
    evaluation::{
        self, request::EvaluationReq, response::Response as EvalRes,
        EvaluationResponse,
    },
    governance::{model::Validation, Quorum},
    validation::request::ValidationReq,
    Error, Governance, Signature, Signed, Subject,
};
use crate::{
    governance, EventRequest, Node, NodeMessage, NodeResponse, SubjectCommand,
    SubjectResponse, DIGEST_DERIVATOR,
};

pub mod approver;
pub mod request;
pub mod response;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Approval {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,

    request_id: String,
    request: Option<Signed<ApprovalReq>>,
    // approvers
    approvers: HashSet<KeyIdentifier>,
    // Actual responses
    approvers_response: Vec<ApprovalRes>,
    // approvers quantity
    approvers_quantity: u32,
}

impl Approval {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Approval {
            node_key,
            ..Default::default()
        }
    }

    // generate the approval request
    async fn create_approval_req(
        &mut self,
        ctx: &mut ActorContext<Approval>,
        req_evaluation: EvaluationReq,
        res_evaluation: EvalRes,
    ) -> Result<ApprovalReq, Error> {
        let subject_id = if let EventRequest::Fact(event) =
            req_evaluation.event_request.content.clone()
        {
            event.subject_id
        } else {
            // Error evento incorrecto
            todo!()
        };

        // Get the derivator of the node
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            derivator.clone()
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };
        // Obtain the last event of subject actor
        let subject_event_path = ctx.path().parent();
        let subject_event_actor: Option<ActorRef<LedgerEvent>> =
            ctx.system().get_actor(&subject_event_path).await;

        let subject_event_actor =
            if let Some(subject_event_actor) = subject_event_actor {
                subject_event_actor
            } else {
                return Err(Error::Actor(format!(
                    "The subject actor was not found in the expected path {}",
                    subject_event_path
                )));
            };

        let response = subject_event_actor
            .ask(LedgerEventCommand::GetLastEvent)
            .await;
        let prev_event = match response {
            Ok(LedgerEventResponse::LastEvent(event)) => event,
            _ => todo!(),
        };

        // Hash of the previous event to add to the approval request
        let hash_prev_event = match DigestIdentifier::from_serializable_borsh(
            prev_event, derivator,
        ) {
            Ok(hash) => hash,
            Err(_) => todo!(),
        };

        Ok(ApprovalReq {
            event_request: req_evaluation.event_request,
            sn: req_evaluation.sn,
            gov_version: req_evaluation.gov_version,
            patch: res_evaluation.patch,
            state_hash: res_evaluation.state_hash,
            hash_prev_event,
            subject_id,
        })
    }

    /// TODO refacorizar ya que solo varia la validation stage(se usa en todos los protocolos)
    async fn get_signers_and_quorum(
        &self,
        ctx: &mut ActorContext<Approval>,
        governance: &DigestIdentifier,
        schema_id: &str,
        namespace: Namespace,
    ) -> Result<(HashSet<KeyIdentifier>, Quorum), Error> {
        // Governance path.
        let governance_path =
            ActorPath::from(format!("/user/node/{}", governance));
        // Governance actor.
        let governance_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&governance_path).await;

        // We obtain the actor governance
        let response = if let Some(governance_actor) = governance_actor {
            // We ask a governance
            let response =
                governance_actor.ask(SubjectCommand::GetGovernance).await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a Subject {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The governance actor was not found in the expected path /user/node/{}",
                governance
            )));
        };

        // We handle the possible responses of governance
        match response {
            SubjectResponse::Governance(gov) => {
                match gov.get_quorum_and_signers(Roles::APPROVER, schema_id, namespace) {
                    Ok(quorum_and_signers) => Ok(quorum_and_signers),
                    Err(error) => Err(Error::Actor(format!("The governance encountered problems when getting signers and quorum: {}",error)))
                }
            }
            SubjectResponse::Error(error) => {
                return Err(Error::Actor(format!("The subject encountered problems when getting governance: {}",error)));
            }
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            ))),
        }
    }

    async fn create_approvers(
        &self,
        ctx: &mut ActorContext<Approval>,
        request_id: &str,
        approval_req: Signed<ApprovalReq>,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        let our_key = self.node_key.clone();

        if signer == our_key {
            let approver_actor: Option<ActorRef<Approver>> = ctx
                .system()
                .get_actor(&ActorPath::from(format!(
                    "/user/node/{}/approver",
                    approval_req.content.subject_id
                )))
                .await;
            if let Some(approver_actor) = approver_actor {
                if let Err(e) = approver_actor
                    .tell(ApproverCommand::LocalApproval {
                        request_id: request_id.to_owned(),
                        approval_req: approval_req.content,
                        our_key: signer,
                    })
                    .await
                {
                    return Err(e);
                }
            } else {
                todo!()
            }
        } else {
            // Create Approvers child
            let child = ctx
                .create_child(
                    &format!("{}", signer),
                    Approver::new(request_id.to_owned(), signer.clone()),
                )
                .await;
            let approver_actor = match child {
                Ok(child) => child,
                Err(e) => return Err(e),
            };

            if let Err(e) = approver_actor
                .tell(ApproverCommand::NetworkApproval {
                    request_id: request_id.to_owned(),
                    approval_req: approval_req.clone(),
                    node_key: signer,
                    our_key,
                })
                .await
            {
                return Err(e);
            }
        }

        Ok(())
    }
    fn check_approval(&mut self, approver: KeyIdentifier) -> bool {
        self.approvers.remove(&approver)
    }
}

#[derive(Debug, Clone)]
pub enum ApprovalCommand {
    Create {
        request_id: DigestIdentifier,
        info: EvaluationReq,
        eval_response: EvalRes,
    },
    Response {
        approval_res: ApprovalRes,
        sender: KeyIdentifier,
    },
    ReLaunch,
}

impl Message for ApprovalCommand {}

//
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalEvent {
    SafeState {
        request_id: String,
        // Quorum
        quorum: Quorum,
        request: Option<Signed<ApprovalReq>>,
        // approvers
        approvers: HashSet<KeyIdentifier>,
        // Actual responses
        approvers_response: Vec<ApprovalRes>,
        // approvers quantity
        approvers_quantity: u32,
    },
}

impl Event for ApprovalEvent {}

#[derive(Debug, Clone)]
pub enum ApprovalResponse {
    Error(Error),
    None,
}
impl Response for ApprovalResponse {}

#[async_trait]
impl Actor for Approval {
    type Event = ApprovalEvent;
    type Message = ApprovalCommand;
    type Response = ApprovalResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Starting approval actor with init store.");
        let prefix = ctx.path().parent().key();
        self.init_store("approval", Some(prefix), false, ctx).await
        // Una vez recuperado el estado debemos ver si el propio nodo ha recibido ya ha enviado la respuesta
        // para no levantar un approver
    }
    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        debug!("Stopping approval actor with stop store.");
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<Approval> for Approval {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: ApprovalCommand,
        ctx: &mut ActorContext<Self>,
    ) -> Result<ApprovalResponse, ActorError> {
        match msg {
            ApprovalCommand::ReLaunch => {
                let request = if let Some(request) = self.request.clone() {
                    request
                } else {
                    todo!()
                };

                for signer in self.approvers.clone() {
                    if let Err(error) = self
                        .create_approvers(
                            ctx,
                            &self.request_id,
                            request.clone(),
                            signer,
                        )
                        .await
                    {
                        // Mensaje al padre de error return Err(error);
                        return Err(error);
                    }
                }
            }
            ApprovalCommand::Create {
                request_id,
                info,
                eval_response,
            } => {
                // Creamos una petición de aprobación, miramos quorum y lanzamos approvers
                let approval_req = match self
                    .create_approval_req(ctx, info.clone(), eval_response)
                    .await
                {
                    Ok(approval_req) => approval_req,
                    Err(e) => todo!(),
                };
                // Get signers and quorum
                let (signers, quorum) = match self
                    .get_signers_and_quorum(
                        ctx,
                        &info.context.subject_id,
                        &info.context.schema_id,
                        Namespace::from(info.context.namespace),
                    )
                    .await
                {
                    Ok(signers_quorum) => signers_quorum,
                    Err(e) => {
                        // Mensaje al padre de error return Ok(ApprovalRes::Error(e))
                        todo!()
                    }
                };
                // Update quorum and validators
                let request_id = request_id.to_string();

                let node_path = ActorPath::from("/user/node");
                let node_actor: Option<ActorRef<Node>> =
                    ctx.system().get_actor(&node_path).await;

                // We obtain the validator
                let node_response = if let Some(node_actor) = node_actor {
                    match node_actor
                        .ask(NodeMessage::SignRequest(
                            SignTypesNode::ApprovalReq(approval_req.clone()),
                        ))
                        .await
                    {
                        Ok(response) => response,
                        Err(e) => todo!(),
                    }
                } else {
                    todo!()
                };

                let signature = match node_response {
                    NodeResponse::SignRequest(signature) => signature,
                    NodeResponse::Error(_) => todo!(),
                    _ => todo!(),
                };

                let signed_approval_req: Signed<ApprovalReq> = Signed {
                    content: approval_req,
                    signature,
                };

                for signer in signers {
                    if let Err(error) = self
                        .create_approvers(
                            ctx,
                            &request_id,
                            signed_approval_req.clone(),
                            signer,
                        )
                        .await
                    {
                        // Mensaje al padre de error return Err(error);
                        return Err(error);
                    }
                }

                if let Err(e) = ctx
                    .event(ApprovalEvent::SafeState {
                        request_id: self.request_id.clone(),
                        quorum: self.quorum.clone(),
                        request: self.request.clone(),
                        approvers: self.approvers.clone(),
                        approvers_response: self.approvers_response.clone(),
                        approvers_quantity: self.approvers_quantity.clone(),
                    })
                    .await
                {
                    // TODO error al persistir, propagar hacia arriba
                };
            }
            ApprovalCommand::Response {
                approval_res,
                sender,
            } => {
                if self.check_approval(sender) {
                    match approval_res.clone() {
                        ApprovalRes::Response(sinature, response) => {
                            if response {
                                self.approvers_response.push(approval_res);
                            }
                        }
                        ApprovalRes::TimeOut(_approval_time_out) => {
                            self.approvers_response.push(approval_res)
                        }
                    };

                    if let Err(e) = ctx
                        .event(ApprovalEvent::SafeState {
                            request_id: self.request_id.clone(),
                            quorum: self.quorum.clone(),
                            request: self.request.clone(),
                            approvers: self.approvers.clone(),
                            approvers_response: self.approvers_response.clone(),
                            approvers_quantity: self.approvers_quantity.clone(),
                        })
                        .await
                    {
                        // TODO error al persistir, propagar hacia arriba
                    };

                    // si hemos llegado al quorum y hay suficientes aprobaciones aprobamos...
                    if self.quorum.check_quorum(
                        self.approvers_quantity,
                        self.approvers_response.len() as u32,
                    ) {
                        // TODO PROPAGAR CONCLUSIÖN
                    } else {
                        // TODO PROPAGAR CONCLUSIÖN
                    }
                } else {
                    // TODO la respuesta no es válida, nos ha llegado una validación de alguien que no esperabamos o ya habíamos recibido la respuesta.
                }
            }
        }
        Ok(ApprovalResponse::None)
    }

    async fn on_event(
        &mut self,
        event: ApprovalEvent,
        ctx: &mut ActorContext<Approval>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            // TODO error al persistir, propagar hacia arriba
        };
    }
}

// Debemos persistir quienes han aprobado y quienes no
#[async_trait]
impl PersistentActor for Approval {
    fn apply(&mut self, event: &ApprovalEvent) {
        match event {
            ApprovalEvent::SafeState {
                request_id,
                quorum,
                request,
                approvers,
                approvers_response,
                approvers_quantity,
            } => {
                self.request_id = request_id.clone();
                self.quorum = quorum.clone();
                self.request = request.clone();
                self.approvers = approvers.clone();
                self.approvers_response = approvers_response.clone();
                self.approvers_quantity = approvers_quantity.clone();
            }
        }
    }
}

impl Storable for Approval {}
