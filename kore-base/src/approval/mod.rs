// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ChildAction, Error as ActorError, Handler, Message,
};
use actor::{ActorPath, ActorRef, Event};
use approver::{Approver, ApproverMessage, VotationType};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use request::ApprovalReq;
use response::ApprovalRes;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;

use crate::evaluation::response::EvalLedgerResponse;
use crate::governance::model::Roles;
use crate::model::common::{
    emit_fail, get_sign, get_signers_quorum_gov_version,
};
use crate::model::event::{LedgerValue, ProtocolsSignatures};
use crate::model::{Namespace, SignTypesNode};
use crate::request::manager::{RequestManager, RequestManagerMessage};
use crate::{
    db::Storable, evaluation::request::EvaluationReq, governance::Quorum,
    Signed, Subject,
};
use crate::{EventRequest, SubjectMessage, SubjectResponse};

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
    ) -> Result<ApprovalReq, ActorError> {
        let subject_id = if let EventRequest::Fact(event) =
            eval_req.event_request.content.clone()
        {
            event.subject_id
        } else {
            return Err(ActorError::FunctionalFail("An attempt is being made to approvation an event that is not fact.".to_owned()));
        };

        // Obtain the last event of subject actor
        let subject_path = ctx.path().parent();
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject_actor) = subject_actor {
            subject_actor.ask(SubjectMessage::GetMetadata).await?
        } else {
            return Err(ActorError::NotFound(subject_path));
        };

        let prev_hash = match response {
            SubjectResponse::Metadata(metadata) => metadata.last_event_hash,
            _ => {
                return Err(ActorError::UnexpectedResponse(
                    subject_path,
                    "SubjectResponse::Metadata".to_owned(),
                ))
            }
        };

        let LedgerValue::Patch(patch) = eval_res.value else {
            return Err(ActorError::FunctionalFail(
                "Approvation can not be possible if eval fail".to_owned(),
            ));
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

    async fn create_approvers(
        &self,
        ctx: &mut ActorContext<Approval>,
        request_id: &str,
        approval_req: Signed<ApprovalReq>,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        let our_key = self.node_key.clone();

        if signer == our_key {
            let approver_path = ActorPath::from(format!(
                "/user/node/{}/approver",
                approval_req.content.subject_id
            ));
            let approver_actor: Option<ActorRef<Approver>> =
                ctx.system().get_actor(&approver_path).await;
            if let Some(approver_actor) = approver_actor {
                approver_actor
                    .tell(ApproverMessage::LocalApproval {
                        request_id: request_id.to_owned(),
                        approval_req: approval_req.content,
                        our_key: signer,
                    })
                    .await?
            } else {
                return Err(ActorError::NotFound(approver_path));
            }
        } else {
            // Create Approvers child
            let Ok(child) = ctx
                .create_child(
                    &signer.to_string(),
                    Approver::new(
                        request_id.to_owned(),
                        signer.clone(),
                        approval_req.content.subject_id.to_string(),
                        VotationType::Manual,
                    ),
                )
                .await
            else {
                return Err(ActorError::Create(
                    ctx.path().clone(),
                    signer.to_string(),
                ));
            };

            child
                .tell(ApproverMessage::NetworkApproval {
                    request_id: request_id.to_owned(),
                    approval_req: approval_req.clone(),
                    node_key: signer,
                    our_key,
                })
                .await?;
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
    ) -> Result<(), ActorError> {
        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        if let Some(req_actor) = req_actor {
            req_actor
                .tell(RequestManagerMessage::ApprovalRes {
                    result: response,
                    signatures: self.approvers_response.clone(),
                })
                .await?
        } else {
            return Err(ActorError::NotFound(req_path));
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

#[async_trait]
impl Actor for Approval {
    type Event = ApprovalEvent;
    type Message = ApprovalMessage;
    type Response = ();

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        let prefix = ctx.path().parent().key();
        self.init_store("approval", Some(prefix), false, ctx).await
        // Una vez recuperado el estado debemos ver si el propio nodo ha recibido ya ha enviado la respuesta
        // para no levantar un approver
    }
    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
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
    ) -> Result<(), ActorError> {
        match msg {
            ApprovalMessage::Create {
                request_id,
                eval_req,
                eval_res,
            } => {
                if request_id == self.request_id {
                    let Some(request) = self.request.clone() else {
                        return Ok(());
                    };

                    for signer in self.approvers.clone() {
                        if let Err(e) = self
                            .create_approvers(
                                ctx,
                                &self.request_id,
                                request.clone(),
                                signer,
                            )
                            .await
                        {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                } else {
                    // Creamos una petición de aprobación, miramos quorum y lanzamos approvers
                    let approval_req = match self
                        .create_approval_req(ctx, eval_req.clone(), eval_res)
                        .await
                    {
                        Ok(approval_req) => approval_req,
                        Err(e) => {
                            return Err(emit_fail(ctx, e).await);
                        }
                    };
                    // Get signers and quorum
                    let (signers, quorum, _) =
                        match get_signers_quorum_gov_version(
                            ctx,
                            &eval_req.context.subject_id.to_string(),
                            &eval_req.context.schema_id,
                            Namespace::from(eval_req.context.namespace),
                            Roles::APPROVER,
                        )
                        .await
                        {
                            Ok(signers_quorum) => signers_quorum,
                            Err(e) => {
                                return Err(emit_fail(ctx, e).await);
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
                        Err(e) => {
                            return Err(emit_fail(ctx, e).await);
                        }
                    };

                    let signed_approval_req: Signed<ApprovalReq> = Signed {
                        content: approval_req,
                        signature,
                    };

                    for signer in signers.clone() {
                        if let Err(e) = self
                            .create_approvers(
                                ctx,
                                &request_id,
                                signed_approval_req.clone(),
                                signer,
                            )
                            .await
                        {
                            return Err(emit_fail(ctx, e).await);
                        }
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
                        if let Err(e) =
                            self.send_approval_to_req(ctx, true).await
                        {
                            return Err(emit_fail(ctx, e).await);
                        };
                    } else if self.approvers.is_empty() {
                        if let Err(e) =
                            self.send_approval_to_req(ctx, false).await
                        {
                            return Err(emit_fail(ctx, e).await);
                        };
                    }
                } else {
                    // TODO la respuesta no es válida, nos ha llegado una validación de alguien que no esperabamos o ya habíamos recibido la respuesta.
                }
            }
        }
        Ok(())
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

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Approval>,
    ) -> ChildAction {
        emit_fail(ctx, error).await;
        ChildAction::Stop
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
