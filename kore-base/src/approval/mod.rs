// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{
    Actor, ActorContext, Error as ActorError, Handler, Message, Response,
};
use actor::{ActorPath, ActorRef, Event};
use approver::{Approver, ApproverMessage, VotationType};
use async_trait::async_trait;
use identity::identifier::derive::digest::DigestDerivator;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use request::ApprovalReq;
use response::ApprovalRes;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{debug, error};

use crate::evaluation::response::EvalLedgerResponse;
use crate::governance::model::Roles;
use crate::model::common::get_sign;
use crate::model::event::{LedgerValue, ProtocolsSignatures};
use crate::model::{Namespace, SignTypesNode};
use crate::request::manager::{RequestManager, RequestManagerMessage};
use crate::subject::event::{
    LedgerEvent, LedgerEventMessage, LedgerEventResponse,
};
use crate::{
    db::Storable, evaluation::request::EvaluationReq, governance::Quorum,
    Error, Signed, Subject,
};
use crate::{EventRequest, SubjectMessage, SubjectResponse, DIGEST_DERIVATOR};

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
    approvers_response: Vec<ProtocolsSignatures>,
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
        eval_req: EvaluationReq,
        eval_res: EvalLedgerResponse,
    ) -> Result<ApprovalReq, Error> {
        let subject_id = if let EventRequest::Fact(event) =
            eval_req.event_request.content.clone()
        {
            event.subject_id
        } else {
            // Error evento incorrecto
            todo!()
        };

        // Obtain the last event of subject actor
        let subject_path = ctx.path().parent();
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject_actor) = subject_actor {
            if let Ok(response) =
                subject_actor.ask(SubjectMessage::GetMetadata).await
            {
                response
            } else {
                todo!()
            }
        } else {
            todo!()
        };

        let prev_hash = match response {
            SubjectResponse::Metadata(metadata) => metadata.last_event_hash,
            _ => todo!(),
        };

        let patch = if let LedgerValue::Patch(value) = eval_res.value {
            value
        } else {
            todo!()
        };

        Ok(ApprovalReq {
            event_request: eval_req.event_request,
            sn: eval_req.sn,
            gov_version: eval_req.gov_version,
            patch,
            state_hash: eval_res.state_hash,
            hash_prev_event: prev_hash,
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
                governance_actor.ask(SubjectMessage::GetGovernance).await;
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
            SubjectResponse::Error(error) => Err(Error::Actor(format!(
                "The subject encountered problems when getting governance: {}",
                error
            ))),
            _ => Err(Error::Actor(
                "An unexpected response has been received from node actor"
                    .to_owned(),
            )),
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
                approver_actor
                    .tell(ApproverMessage::LocalApproval {
                        request_id: request_id.to_owned(),
                        approval_req: approval_req.content,
                        our_key: signer,
                    })
                    .await?
            } else {
                todo!()
            }
        } else {
            // Create Approvers child
            let child = ctx
                .create_child(
                    &format!("{}", signer),
                    Approver::new(
                        request_id.to_owned(),
                        signer.clone(),
                        approval_req.content.subject_id.to_string(),
                        VotationType::Manual,
                    ),
                )
                .await;
            let approver_actor = match child {
                Ok(child) => child,
                Err(_e) => return Err(_e),
            };

            approver_actor
                .tell(ApproverMessage::NetworkApproval {
                    request_id: request_id.to_owned(),
                    approval_req: approval_req.clone(),
                    node_key: signer,
                    our_key,
                })
                .await?
        }

        Ok(())
    }
    fn check_approval(&mut self, approver: KeyIdentifier) -> bool {
        self.approvers.remove(&approver)
    }

    async fn send_approval_to_req(
        &self,
        ctx: &mut ActorContext<Approval>,
        response: bool,
    ) -> Result<(), Error> {
        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        if let Some(req_actor) = req_actor {
            if let Err(_e) = req_actor
                .tell(RequestManagerMessage::ApprovalRes {
                    result: response,
                    signatures: self.approvers_response.clone(),
                })
                .await
            {
                todo!()
            }
        } else {
            todo!()
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ApprovalMessage {
    Create {
        request_id: String,
        eval_req: EvaluationReq,
        eval_res: EvalLedgerResponse,
    },
    Response {
        approval_res: ApprovalRes,
        sender: KeyIdentifier,
    },
}

impl Message for ApprovalMessage {}

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
        approvers_response: Vec<ProtocolsSignatures>,
        // approvers quantity
        approvers_quantity: u32,
    },
    Response(ProtocolsSignatures),
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
    type Message = ApprovalMessage;
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
        __sender: ActorPath,
        msg: ApprovalMessage,
        ctx: &mut ActorContext<Self>,
    ) -> Result<ApprovalResponse, ActorError> {
        match msg {
            ApprovalMessage::Create {
                request_id,
                eval_req,
                eval_res,
            } => {
                if request_id == self.request_id {
                    let request = if let Some(request) = self.request.clone() {
                        request
                    } else {
                        todo!()
                    };

                    for signer in self.approvers.clone() {
                        self.create_approvers(
                            ctx,
                            &self.request_id,
                            request.clone(),
                            signer,
                        )
                        .await?
                    }
                } else {
                    // Creamos una petición de aprobación, miramos quorum y lanzamos approvers
                    let approval_req = match self
                        .create_approval_req(ctx, eval_req.clone(), eval_res)
                        .await
                    {
                        Ok(approval_req) => approval_req,
                        Err(e) => {
                            todo!()
                        }
                    };
                    // Get signers and quorum
                    let (signers, quorum) = match self
                        .get_signers_and_quorum(
                            ctx,
                            &eval_req.context.subject_id,
                            &eval_req.context.schema_id,
                            Namespace::from(eval_req.context.namespace),
                        )
                        .await
                    {
                        Ok(signers_quorum) => signers_quorum,
                        Err(_e) => {
                            // Mensaje al padre de error return Ok(ApprovalRes::Error(e))
                            todo!()
                        }
                    };
                    // Update quorum and validators
                    let request_id = request_id.to_string();

                    let signature = match get_sign(
                        ctx,
                        SignTypesNode::ApprovalReq(approval_req.clone()),
                    )
                    .await
                    {
                        Ok(signature) => signature,
                        Err(_e) => todo!(),
                    };

                    let signed_approval_req: Signed<ApprovalReq> = Signed {
                        content: approval_req,
                        signature,
                    };

                    for signer in signers.clone() {
                        self.create_approvers(
                            ctx,
                            &request_id,
                            signed_approval_req.clone(),
                            signer,
                        )
                        .await?
                    }

                    self.on_event(
                        ApprovalEvent::SafeState {
                            request_id: request_id.clone(),
                            quorum: quorum.clone(),
                            request: Some(signed_approval_req.clone()),
                            approvers: signers.clone(),
                            approvers_response: vec![].clone(),
                            approvers_quantity: signers.len() as u32,
                        },
                        ctx,
                    )
                    .await;
                }
            }
            ApprovalMessage::Response {
                approval_res,
                sender,
            } => {
                if self.check_approval(sender) {
                    match approval_res.clone() {
                        ApprovalRes::Response(sinature, response) => {
                            if response {
                                self.on_event(
                                    ApprovalEvent::Response(
                                        ProtocolsSignatures::Signature(
                                            sinature,
                                        ),
                                    ),
                                    ctx,
                                )
                                .await;
                            }
                        }
                        ApprovalRes::TimeOut(approval_time_out) => {
                            self.on_event(
                                ApprovalEvent::Response(
                                    ProtocolsSignatures::TimeOut(
                                        approval_time_out,
                                    ),
                                ),
                                ctx,
                            )
                            .await;
                        }
                    };

                    // si hemos llegado al quorum y hay suficientes aprobaciones aprobamos...
                    if self.quorum.check_quorum(
                        self.approvers_quantity,
                        self.approvers_response.len() as u32,
                    ) {
                        if let Err(_e) =
                            self.send_approval_to_req(ctx, true).await
                        {
                            todo!()
                        };
                    } else if self.approvers.is_empty() {
                        if let Err(_e) =
                            self.send_approval_to_req(ctx, false).await
                        {
                            todo!()
                        };
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
        if let Err(_e) = self.persist(&event, ctx).await {
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
                self.request_id.clone_from(request_id);
                self.quorum = quorum.clone();
                self.request.clone_from(request);
                self.approvers.clone_from(approvers);
                self.approvers_response.clone_from(approvers_response);
                self.approvers_quantity = *approvers_quantity;
            }
            ApprovalEvent::Response(response) => {
                self.approvers_response.push(response.clone());
            }
        }
    }
}

impl Storable for Approval {}
