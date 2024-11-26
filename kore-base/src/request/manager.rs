// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use store::store::PersistentActor;
use tracing::error;

use crate::{
    approval::{Approval, ApprovalMessage},
    db::Storable,
    distribution::{Distribution, DistributionMessage},
    evaluation::{
        request::EvaluationReq, response::EvalLedgerResponse, Evaluation,
        EvaluationMessage,
    },
    model::{
        common::{
            change_temp_subj, emit_fail, get_gov, get_metadata, get_sign, update_event
        },
        event::{
            DataProofEvent, Ledger, LedgerValue, ProofEvent, ProtocolsError,
            ProtocolsSignatures,
        },
        SignTypesNode,
    },
    validation::proof::EventProof,
    Error, Event as KoreEvent, EventRequest, HashId, Signed, Subject,
    SubjectMessage, SubjectResponse, Validation, ValidationInfo,
    ValidationMessage, ValueWrapper, DIGEST_DERIVATOR,
};

use super::{
    state::RequestManagerState, RequestHandler, RequestHandlerMessage,
};

#[derive(Default)]
pub struct ProtocolsResult {
    pub eval_success: Option<bool>,
    pub appr_required: bool,
    pub appr_success: Option<bool>,
    pub eval_signatures: Option<HashSet<ProtocolsSignatures>>,
    pub appr_signatures: Option<HashSet<ProtocolsSignatures>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestManager {
    id: String,
    state: RequestManagerState,
    subject_id: String,
    request: Signed<EventRequest>,
}

impl RequestManager {
    pub fn new(
        id: String,
        subject_id: String,
        request: Signed<EventRequest>,
    ) -> Self {
        RequestManager {
            id,
            state: RequestManagerState::Starting,
            subject_id,
            request,
        }
    }
    async fn send_validation(
        &self,
        ctx: &mut ActorContext<RequestManager>,
        val_info: ValidationInfo,
    ) -> Result<(), ActorError> {
        let validation_path = ActorPath::from(format!(
            "/user/node/{}/validation",
            self.subject_id
        ));
        let validation_actor: Option<ActorRef<Validation>> =
            ctx.system().get_actor(&validation_path).await;

        if let Some(validation_actor) = validation_actor {
            if let Err(e) = validation_actor
                .tell(ValidationMessage::Create {
                    request_id: self.id.clone(),
                    info: val_info,
                })
                .await
            {
                return Err(e);
            }
        } else {
            return Err(ActorError::NotFound(validation_path));
        }

        Ok(())
    }

    async fn send_evaluation(
        &self,
        ctx: &mut ActorContext<RequestManager>,
    ) -> Result<(), ActorError> {
        let evaluation_path = ActorPath::from(format!(
            "/user/node/{}/evaluation",
            self.subject_id
        ));
        let evaluation_actor: Option<ActorRef<Evaluation>> =
            ctx.system().get_actor(&evaluation_path).await;

        if let Some(evaluation_actor) = evaluation_actor {
            if let Err(e) = evaluation_actor
                .tell(EvaluationMessage::Create {
                    request_id: self.id.clone(),
                    request: self.request.clone(),
                })
                .await
            {
                return Err(e);
            }
        } else {
            return Err(ActorError::NotFound(evaluation_path));
        }

        Ok(())
    }

    async fn send_approval(
        &self,
        ctx: &mut ActorContext<RequestManager>,
        eval_req: EvaluationReq,
        eval_res: EvalLedgerResponse,
    ) -> Result<(), ActorError> {
        let approval_path =
            ActorPath::from(format!("/user/node/{}/approval", self.subject_id));
        let approval_actor: Option<ActorRef<Approval>> =
            ctx.system().get_actor(&approval_path).await;

        if let Some(approval_actor) = approval_actor {
            if let Err(e) = approval_actor
                .tell(ApprovalMessage::Create {
                    request_id: self.id.clone(),
                    eval_req,
                    eval_res,
                })
                .await
            {
                return Err(e);
            }
        } else {
            return Err(ActorError::NotFound(approval_path));
        }

        Ok(())
    }

    async fn validation(
        &mut self,
        ctx: &mut ActorContext<RequestManager>,
        data: DataProofEvent,
    ) -> Result<(), ActorError> {
        let event_proof = EventProof::from(self.request.content.clone());

        let event_proof = ProofEvent {
            subject_id: data.metadata.subject_id.clone(),
            event_proof,
            sn: data.sn,
            gov_version: data.gov_version,
            value: data.value,
            state_hash: data.state_hash,
            eval_success: data.eval_success,
            appr_required: data.appr_required,
            appr_success: data.appr_success,
            hash_prev_event: data.metadata.last_event_hash.clone(),
            evaluators: data.eval_signatures,
            approvers: data.appr_signatures,
        };

        let signature = get_sign(
            ctx,
            SignTypesNode::ValidationProofEvent(event_proof.clone()),
        )
        .await?;

        let event_proof = Signed {
            content: event_proof,
            signature,
        };

        let val_info = ValidationInfo {
            metadata: data.metadata,
            event_proof,
        };

        self.on_event(
            RequestManagerEvent {
                id: self.id.clone(),
                state: RequestManagerState::Validation(val_info.clone()),
            },
            ctx,
        )
        .await;

        self.send_validation(ctx, val_info.clone()).await
    }

    async fn approval(
        &mut self,
        ctx: &mut ActorContext<RequestManager>,
        eval_req: EvaluationReq,
        eval_res: EvalLedgerResponse,
        eval_signatures: HashSet<ProtocolsSignatures>,
    ) -> Result<(), ActorError> {
        self.on_event(
            RequestManagerEvent {
                id: self.id.clone(),
                state: RequestManagerState::Approval {
                    eval_req: eval_req.clone(),
                    eval_res: eval_res.clone(),
                    eval_signatures,
                },
            },
            ctx,
        )
        .await;

        self.send_approval(ctx, eval_req, eval_res).await
    }

    async fn evaluation(
        &mut self,
        ctx: &mut ActorContext<RequestManager>,
    ) -> Result<(), ActorError> {
        self.on_event(
            RequestManagerEvent {
                id: self.id.clone(),
                state: RequestManagerState::Evaluation,
            },
            ctx,
        )
        .await;

        self.send_evaluation(ctx).await
    }

    fn create_ledger_event(
        &self,
        val_info: ValidationInfo,
        signatures: Vec<ProtocolsSignatures>,
        result: bool,
        errors: &str,
    ) -> (Ledger, KoreEvent) {
        let value = {
            if result {
                val_info.event_proof.content.value
            } else if let LedgerValue::Error(mut e) =
                val_info.event_proof.content.value
            {
                e.validation = Some(errors.to_owned());
                LedgerValue::Error(e)
            } else {
                let e = ProtocolsError {
                    evaluation: None,
                    validation: Some(errors.to_owned()),
                };
                LedgerValue::Error(e)
            }
        };

        let event = KoreEvent {
            subject_id: val_info.event_proof.content.subject_id,
            event_request: self.request.clone(),
            sn: val_info.event_proof.content.sn,
            gov_version: val_info.event_proof.content.gov_version,
            value,
            state_hash: val_info.event_proof.content.state_hash,
            eval_success: val_info.event_proof.content.eval_success,
            appr_required: val_info.event_proof.content.appr_required,
            appr_success: val_info.event_proof.content.appr_success,
            vali_success: result,
            hash_prev_event: val_info.event_proof.content.hash_prev_event,
            evaluators: val_info.event_proof.content.evaluators,
            approvers: val_info.event_proof.content.approvers,
            validators: HashSet::from_iter(signatures.iter().cloned()),
        };

        (Ledger::from(event.clone()), event)
    }

    async fn safe_ledger_event(
        &mut self,
        ctx: &mut ActorContext<RequestManager>,
        event: KoreEvent,
        ledger: Ledger,
    ) -> Result<(), ActorError> {
        let signature_ledger =
            get_sign(ctx, SignTypesNode::Ledger(ledger.clone())).await?;

        let signed_ledger = Signed {
            content: ledger,
            signature: signature_ledger,
        };

        RequestManager::update_ledger(ctx, signed_ledger.clone()).await?;

        let signature_event =
            get_sign(ctx, SignTypesNode::Event(event.clone())).await?;

        let signed_event = Signed {
            content: event,
            signature: signature_event,
        };

        update_event(ctx, signed_event.clone()).await?;

        if let EventRequest::Create(_) = self.request.content {
            change_temp_subj(
                ctx,
                signed_event.content.subject_id.to_string(),
                signed_event.signature.signer.to_string(),
            )
            .await?;
        }

        self.on_event(
            RequestManagerEvent {
                id: self.id.clone(),
                state: RequestManagerState::Distribution {
                    event: signed_event.clone(),
                    ledger: signed_ledger.clone(),
                },
            },
            ctx,
        )
        .await;

        self
            .init_distribution(ctx, signed_event, signed_ledger)
            .await
    }

    async fn init_distribution(
        &self,
        ctx: &mut ActorContext<RequestManager>,
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
    ) -> Result<(), ActorError> {
        let distribution_path = ActorPath::from(format!(
            "/user/node/{}/distribution",
            event.content.subject_id
        ));
        let distribution_actor: Option<ActorRef<Distribution>> =
            ctx.system().get_actor(&distribution_path).await;

        if let Some(distribution_actor) = distribution_actor {
            distribution_actor
                .tell(DistributionMessage::Create {
                    request_id: self.id.clone(),
                    event,
                    ledger,
                })
                .await?
        } else {
            return Err(ActorError::NotFound(distribution_path));
        };

        Ok(())
    }

    async fn update_ledger(
        ctx: &mut ActorContext<RequestManager>,
        ledger: Signed<Ledger>,
    ) -> Result<(), ActorError> {
        let subject_path = ActorPath::from(format!(
            "/user/node/{}",
            ledger.content.subject_id
        ));
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject_actor) = subject_actor {
            subject_actor
                .ask(SubjectMessage::UpdateLedger {
                    events: vec![ledger],
                })
                .await?
        } else {
            return Err(ActorError::NotFound(subject_path));
        };

        match response {
            SubjectResponse::LastSn(_) => Ok(()),
            _ => Err(ActorError::UnexpectedResponse(subject_path, "SubjectResponse::LastSn".to_owned())),
        }
    }

    async fn end_request(
        &self,
        ctx: &mut ActorContext<RequestManager>,
    ) -> Result<(), ActorError> {
        let request_path = ActorPath::from("/user/request");
        let request_actor: Option<ActorRef<RequestHandler>> =
            ctx.system().get_actor(&request_path).await;

        if let Some(request_actor) = request_actor {
            request_actor
                .tell(RequestHandlerMessage::EndHandling {
                    id: self.id.clone(),
                    subject_id: self.subject_id.to_string(),
                })
                .await?
        } else {
            return Err(ActorError::NotFound(request_path));
        };

        Ok(())
    }

    async fn build_data_event_proof(
        &self,
        ctx: &mut ActorContext<RequestManager>,
        sn: Option<u64>,
        value: LedgerValue,
        state_hash: Option<DigestIdentifier>,
        protocols_result: ProtocolsResult,
    ) -> Result<DataProofEvent, ActorError> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };

        let gov = get_gov(ctx, &self.subject_id).await?;

        let metadata = get_metadata(ctx, &self.subject_id).await?;

        let state_hash = if let Some(state_hash) = state_hash {
            state_hash
        } else {
            metadata.properties.hash_id(derivator).map_err(|e| ActorError::FunctionalFail(format!("Can not obtain hash id for metadata propierties: {}", e.to_string())))?
        };

        let sn = if let Some(sn) = sn {
            sn
        } else {
            if metadata.sn == 0 {
                if let EventRequest::Create(_) = self.request.content {
                    metadata.sn
                } else {
                    metadata.sn + 1
                }
            } else {
                metadata.sn + 1
            }
        };

        Ok(DataProofEvent {
            gov_version: gov.version,
            metadata,
            sn,
            eval_success: protocols_result.eval_success,
            appr_required: protocols_result.appr_required,
            appr_success: protocols_result.appr_success,
            value,
            state_hash,
            eval_signatures: protocols_result.eval_signatures,
            appr_signatures: protocols_result.appr_signatures,
        })
    }
}

#[derive(Debug, Clone)]
pub enum RequestManagerMessage {
    Run,
    Reboot,
    Fact,
    Other,
    ApprovalRes {
        result: bool,
        signatures: Vec<ProtocolsSignatures>,
    },
    ValidationRes {
        result: bool,
        signatures: Vec<ProtocolsSignatures>,
        errors: String,
    },
    EvaluationRes {
        request: EvaluationReq,
        response: EvalLedgerResponse,
        signatures: Vec<ProtocolsSignatures>,
    },
    FinishRequest,
}

impl Message for RequestManagerMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestManagerEvent {
    pub id: String,
    pub state: RequestManagerState,
}

impl Event for RequestManagerEvent {}

#[async_trait]
impl Actor for RequestManager {
    type Event = RequestManagerEvent;
    type Message = RequestManagerMessage;
    type Response = ();

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("request_manager", None, false, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<RequestManager> for RequestManager {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: RequestManagerMessage,
        ctx: &mut actor::ActorContext<RequestManager>,
    ) -> Result<(), ActorError> {
        match msg {
            RequestManagerMessage::Reboot => {
                match self.request.content {
                    EventRequest::Fact(_) => {
                        if let Err(e) = self.evaluation(ctx).await {
                            return Err(emit_fail(ctx, e).await);
                        };
                    }
                    _ => {
                        let data = match self
                            .build_data_event_proof(
                                ctx,
                                None,
                                LedgerValue::Patch(ValueWrapper(
                                    serde_json::Value::String("[]".to_owned()),
                                )),
                                None,
                                ProtocolsResult::default(),
                            )
                            .await
                        {
                            Ok(data) => data,
                            Err(e) => return Err(emit_fail(ctx, e).await),
                        };

                        if let Err(e) = self.validation(ctx, data).await {
                            return Err(emit_fail(ctx, e).await);
                        };
                    }
                };
            }
            RequestManagerMessage::Run => {
                match self.state.clone() {
                    RequestManagerState::Starting => {
                        match self.request.content {
                            EventRequest::Fact(_) => {
                                if let Err(e) = self.evaluation(ctx).await {
                                    return Err(emit_fail(ctx, e).await);
                                };
                            }
                            _ => {
                                let data = match self
                                    .build_data_event_proof(
                                        ctx,
                                        None,
                                        LedgerValue::Patch(ValueWrapper(
                                            serde_json::Value::String(
                                                "[]".to_owned(),
                                            ),
                                        )),
                                        None,
                                        ProtocolsResult::default(),
                                    )
                                    .await
                                {
                                    Ok(data) => data,
                                    Err(e) => return Err(emit_fail(ctx, e).await),
                                };

                                if let Err(e) =
                                    self.validation(ctx, data).await
                                {
                                    return Err(emit_fail(ctx, e).await);
                                };
                            }
                        };
                    }
                    RequestManagerState::Evaluation => {
                        if let Err(e) = self.send_evaluation(ctx).await {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                    RequestManagerState::Approval {
                        eval_req,
                        eval_res,
                        ..
                    } => {
                        if let Err(e) =
                            self.send_approval(ctx, eval_req, eval_res).await
                        {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                    RequestManagerState::Validation(val_info) => {
                        if let Err(e) =
                            self.send_validation(ctx, val_info).await
                        {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                    RequestManagerState::Distribution { event, ledger } => {
                        if let Err(e) =
                            self.init_distribution(ctx, event, ledger).await
                        {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                };
            }
            RequestManagerMessage::ApprovalRes { result, signatures } => {
                let (eval_req, eval_res, eval_signatures) =
                    if let RequestManagerState::Approval {
                        eval_req,
                        eval_res,
                        eval_signatures,
                    } = self.state.clone()
                    {
                        (eval_req, eval_res, eval_signatures)
                    } else {
                        let e = ActorError::FunctionalFail("Invalid request state".to_owned());
                        return Err(emit_fail(ctx, e).await);
                    };

                let data = match self
                    .build_data_event_proof(
                        ctx,
                        Some(eval_req.sn),
                        eval_res.value,
                        Some(eval_res.state_hash),
                        ProtocolsResult {
                            eval_success: Some(eval_res.eval_success),
                            appr_required: eval_res.appr_required,
                            appr_success: Some(result),
                            eval_signatures: Some(eval_signatures),
                            appr_signatures: Some(HashSet::from_iter(
                                signatures.iter().cloned(),
                            )),
                        },
                    )
                    .await
                {
                    Ok(data) => data,
                    Err(e) => return Err(emit_fail(ctx, e).await),
                };

                if let Err(e) = self.validation(ctx, data).await {
                    return Err(emit_fail(ctx, e).await);
                }
            }
            RequestManagerMessage::EvaluationRes {
                request,
                response,
                signatures,
            } => {
                if let RequestManagerState::Evaluation = self.state.clone() {
                } else {
                    let e = ActorError::FunctionalFail("Invalid request state".to_owned());
                    return Err(emit_fail(ctx, e).await);
                };

                if response.appr_required {
                    if let Err(_e) = self
                        .approval(
                            ctx,
                            request,
                            response,
                            HashSet::from_iter(signatures.iter().cloned()),
                        )
                        .await
                    {}
                } else {
                    let data = match self
                        .build_data_event_proof(
                            ctx,
                            Some(request.sn),
                            response.value,
                            Some(response.state_hash),
                            ProtocolsResult {
                                eval_success: Some(response.eval_success),
                                appr_required: response.appr_required,
                                appr_success: None,
                                eval_signatures: Some(HashSet::from_iter(
                                    signatures.iter().cloned(),
                                )),
                                appr_signatures: None,
                            },
                        )
                        .await
                    {
                        Ok(data) => data,
                        Err(e) => return Err(emit_fail(ctx, e).await),
                    };

                    if let Err(e) = self.validation(ctx, data).await {
                        return Err(emit_fail(ctx, e).await);
                    }
                }
            }
            RequestManagerMessage::FinishRequest => {
                if let RequestManagerState::Distribution { .. } =
                    self.state.clone()
                {
                } else {
                    let e = ActorError::FunctionalFail("Invalid request state".to_owned());
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = self.end_request(ctx).await {
                    return Err(emit_fail(ctx, e).await);
                }

                // TODO Limpiar la base de datos.
                ctx.stop().await;
            }
            RequestManagerMessage::ValidationRes {
                result,
                signatures,
                errors,
            } => {
                let actual_state =
                    if let RequestManagerState::Validation(state) =
                        self.state.clone()
                    {
                        state
                    } else {
                        let e = ActorError::FunctionalFail("Invalid request state".to_owned());
                        return Err(emit_fail(ctx, e).await);
                    };

                let (ledger, event) = self.create_ledger_event(
                    actual_state,
                    signatures,
                    result,
                    &errors,
                );

                if let Err(e) =
                    self.safe_ledger_event(ctx, event, ledger).await
                {
                    return Err(emit_fail(ctx, e).await);
                }
            }
            RequestManagerMessage::Fact => {
                if let RequestManagerState::Starting = self.state {
                } else {
                    let e = ActorError::FunctionalFail("Invalid request state".to_owned());
                    return Err(emit_fail(ctx, e).await);
                };

                if let Err(e) = self.evaluation(ctx).await {
                    return Err(emit_fail(ctx, e).await);
                };
            }
            RequestManagerMessage::Other => {
                if let RequestManagerState::Starting = self.state {
                } else {
                    let e = ActorError::FunctionalFail("Invalid request state".to_owned());
                    return Err(emit_fail(ctx, e).await);
                };

                let data = match self
                    .build_data_event_proof(
                        ctx,
                        None,
                        LedgerValue::Patch(ValueWrapper(
                            serde_json::Value::String("[]".to_owned()),
                        )),
                        None,
                        ProtocolsResult::default(),
                    )
                    .await
                {
                    Ok(data) => data,
                    Err(e) => return Err(emit_fail(ctx, e).await),
                };

                if let Err(e) = self.validation(ctx, data).await {
                    return Err(emit_fail(ctx, e).await);
                };
            }
        }

        Ok(())
    }

    async fn on_event(
        &mut self,
        event: RequestManagerEvent,
        ctx: &mut ActorContext<RequestManager>,
    ) {
        if let Err(_e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };

        if let Err(e) = ctx.publish_event(event).await {};
    }
}

#[async_trait]
impl PersistentActor for RequestManager {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        self.state = event.state.clone();
    }
}

#[async_trait]
impl Storable for RequestManager {}
