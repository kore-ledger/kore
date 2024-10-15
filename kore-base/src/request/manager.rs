// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use identity::{
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::Metadata,
};
use store::store::PersistentActor;
use tracing::error;

use crate::{
    approval::{response, Approval, ApprovalMessage},
    db::Storable,
    distribution::{Distribution, DistributionMessage},
    evaluation::{
        request::EvaluationReq, response::EvalLedgerResponse, Evaluation,
        EvaluationMessage,
    },
    governance::model::Roles,
    init_state,
    model::{
        common::{get_gov, get_metadata, get_sign, update_event},
        event::{
            DataProofEvent, Ledger, LedgerValue, ProofEvent, ProtocolsError,
            ProtocolsSignatures,
        },
        signature, SignTypesNode,
    },
    node,
    subject::{self, CreateSubjectData, SubjectID, SubjectMetadata},
    validation::{proof::EventProof, ValidationEvent},
    CreateRequest, Error, Event as KoreEvent, EventRequest, FactRequest,
    HashId, Node, NodeMessage, NodeResponse, Signature, Signed, Subject,
    SubjectMessage, SubjectResponse, Validation, ValidationInfo,
    ValidationMessage, ValueWrapper, DIGEST_DERIVATOR,
};

use super::{state::RequestSate, RequestHandler, RequestHandlerMessage};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestManager {
    id: String,
    state: RequestSate,
    subject_id: String,
    request: Signed<EventRequest>,
}

impl RequestManager {
    async fn send_validation(
        &self,
        ctx: &mut ActorContext<RequestManager>,
        val_info: ValidationInfo,
    ) -> Result<(), Error> {
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
                todo!()
            }
        } else {
            todo!()
        }

        Ok(())
    }

    async fn send_evaluation(
        &self,
        ctx: &mut ActorContext<RequestManager>,
    ) -> Result<(), Error> {
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
                todo!()
            }
        } else {
            todo!()
        }

        Ok(())
    }

    async fn send_approval(
        &self,
        ctx: &mut ActorContext<RequestManager>,
        eval_req: EvaluationReq,
        eval_res: EvalLedgerResponse,
    ) -> Result<(), Error> {
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
                todo!()
            }
        } else {
            todo!()
        }

        Ok(())
    }

    async fn validation(
        &mut self,
        ctx: &mut ActorContext<RequestManager>,
        data: DataProofEvent,
    ) -> Result<(), Error> {
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

        let signature = match get_sign(
            ctx,
            SignTypesNode::ValidationProofEvent(event_proof.clone()),
        )
        .await
        {
            Ok(signature) => signature,
            Err(e) => todo!(),
        };

        let event_proof = Signed {
            content: event_proof,
            signature,
        };

        let val_info = ValidationInfo {
            metadata: data.metadata,
            event_proof,
        };

        self.on_event(
            RequestManagerEvent::ChangeState {
                state: RequestSate::Validation(val_info.clone()),
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
    ) -> Result<(), Error> {
        self.on_event(
            RequestManagerEvent::ChangeState {
                state: RequestSate::Approval {
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
    ) -> Result<(), Error> {
        self.on_event(
            RequestManagerEvent::ChangeState {
                state: RequestSate::Evaluation,
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
            } else {
                if let LedgerValue::Error(mut e) =
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
            }
        };

        let event = KoreEvent {
            subject_id: val_info.event_proof.content.subject_id,
            event_request: self.request.clone(),
            sn: val_info.event_proof.content.sn,
            gov_version: val_info.event_proof.content.gov_version,
            value,
            state_hash: val_info.event_proof.content.state_hash,
            eval_success: None,
            appr_required: false,
            appr_success: None,
            vali_success: result,
            hash_prev_event: val_info.event_proof.content.hash_prev_event,
            evaluators: None,
            approvers: None,
            validators: HashSet::from_iter(signatures.iter().cloned()),
        };

        (Ledger::from(event.clone()), event)
    }

    async fn safe_ledger_event(
        &mut self,
        ctx: &mut ActorContext<RequestManager>,
        event: KoreEvent,
        ledger: Ledger,
    ) -> Result<(), Error> {
        let signature_ledger =
            get_sign(ctx, SignTypesNode::Ledger(ledger.clone())).await;

        let signature_ledger = match signature_ledger {
            Ok(signature) => signature,
            Err(e) => todo!(),
        };

        let signed_ledger = Signed {
            content: ledger,
            signature: signature_ledger,
        };

        if let Err(e) =
            RequestManager::update_ledger(ctx, signed_ledger.clone()).await
        {
        }

        let signature_event =
            get_sign(ctx, SignTypesNode::Event(event.clone())).await;

        let signature_event = match signature_event {
            Ok(signature) => signature,
            Err(e) => todo!(),
        };

        let signed_event = Signed {
            content: event,
            signature: signature_event,
        };

        if let Err(e) = update_event(ctx, signed_event.clone()).await {
            todo!()
        };

        self.on_event(
            RequestManagerEvent::ChangeState {
                state: RequestSate::Distribution {
                    event: signed_event.clone(),
                    ledger: signed_ledger.clone(),
                },
            },
            ctx,
        )
        .await;

        if let Err(e) = self
            .init_distribution(ctx, signed_event, signed_ledger)
            .await
        {
            todo!()
        };

        Ok(())
    }

    async fn init_distribution(
        &self,
        ctx: &mut ActorContext<RequestManager>,
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
    ) -> Result<(), Error> {
        let distribution_path = ActorPath::from(format!(
            "/user/node/{}/distribution",
            event.content.subject_id
        ));
        let distribution_actor: Option<ActorRef<Distribution>> =
            ctx.system().get_actor(&distribution_path).await;

        if let Some(distribution_actor) = distribution_actor {
            if let Err(e) = distribution_actor
                .tell(DistributionMessage::Create {
                    request_id: self.id.clone(),
                    event,
                    ledger,
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

    async fn update_ledger(
        ctx: &mut ActorContext<RequestManager>,
        ledger: Signed<Ledger>,
    ) -> Result<(), Error> {
        let subject_path = ActorPath::from(format!(
            "/user/node/{}",
            ledger.content.subject_id
        ));
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject_actor) = subject_actor {
            if let Ok(res) = subject_actor
                .ask(SubjectMessage::UpdateLedger {
                    events: vec![ledger],
                })
                .await
            {
                res
            } else {
                todo!()
            }
        } else {
            todo!()
        };

        match response {
            SubjectResponse::LastSn(_) => Ok(()),
            SubjectResponse::Error(e) => todo!(),
            _ => todo!(),
        }
    }

    async fn end_request(
        &self,
        ctx: &mut ActorContext<RequestManager>,
    ) -> Result<(), Error> {
        let request_path = ActorPath::from("/user/request");
        let request_actor: Option<ActorRef<RequestHandler>> =
            ctx.system().get_actor(&request_path).await;

        if let Some(request_actor) = request_actor {
            if let Err(e) = request_actor
                .tell(RequestHandlerMessage::EndHandling {
                    subject_id: self.subject_id.to_string(),
                })
                .await
            {}
        };

        Ok(())
    }

    async fn build_data_event_proof(
        &self,
        ctx: &mut ActorContext<RequestManager>,
        sn: Option<u64>,
        value: LedgerValue,
        state_hash: Option<DigestIdentifier>,
        eval_success: Option<bool>,
        appr_required: bool,
        appr_success: Option<bool>,
        eval_signatures: Option<HashSet<ProtocolsSignatures>>,
        appr_signatures: Option<HashSet<ProtocolsSignatures>>,
    ) -> Result<DataProofEvent, Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };

        let gov = match get_gov(ctx, &self.subject_id).await {
            Ok(gov) => gov,
            Err(e) => todo!(),
        };

        let metadata = match get_metadata(ctx, &self.subject_id).await {
            Ok(metadata) => metadata,
            Err(e) => todo!(),
        };

        let state_hash = if let Some(state_hash) = state_hash {
            state_hash
        } else {
            metadata.properties.hash_id(derivator)?
        };

        let sn = if let Some(sn) = sn {
            sn
        } else {
            metadata.sn + 1
        };

        Ok(DataProofEvent {
            gov_version: gov.version,
            metadata,
            sn,
            eval_success,
            appr_required,
            appr_success,
            value,
            state_hash,
            eval_signatures,
            appr_signatures,
        })
    }
}

#[derive(Debug, Clone)]
pub enum RequestManagerMessage {
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

#[derive(Debug, Clone)]
pub enum RequestManagerResponse {
    Ok(String),
    Error(Error),
    None,
}

impl Response for RequestManagerResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestManagerEvent {
    ChangeState { state: RequestSate },
}

impl Event for RequestManagerEvent {}

#[async_trait]
impl Actor for RequestManager {
    type Event = RequestManagerEvent;
    type Message = RequestManagerMessage;
    type Response = RequestManagerResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        // Cuando arranque tiene que ver su estado para saber en que punto se encontraba y retomarlo. TODO
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
impl PersistentActor for RequestManager {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            RequestManagerEvent::ChangeState { state } => {
                self.state = state.clone();
            }
        }
    }
}

#[async_trait]
impl Handler<RequestManager> for RequestManager {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: RequestManagerMessage,
        ctx: &mut actor::ActorContext<RequestManager>,
    ) -> Result<RequestManagerResponse, ActorError> {
        match msg {
            RequestManagerMessage::ApprovalRes { result, signatures } => {
                let (eval_req, eval_res, eval_signatures) =
                    if let RequestSate::Approval {
                        eval_req,
                        eval_res,
                        eval_signatures,
                    } = self.state.clone()
                    {
                        (eval_req, eval_res, eval_signatures)
                    } else {
                        todo!()
                    };

                let data = match self
                    .build_data_event_proof(
                        ctx,
                        Some(eval_req.sn),
                        eval_res.value,
                        Some(eval_res.state_hash),
                        Some(eval_res.eval_success),
                        eval_res.appr_required,
                        Some(result),
                        Some(eval_signatures),
                        Some(HashSet::from_iter(signatures.iter().cloned())),
                    )
                    .await
                {
                    Ok(data) => data,
                    Err(e) => todo!(),
                };

                if let Err(e) = self.validation(ctx, data).await {
                    todo!()
                }
            }
            RequestManagerMessage::EvaluationRes {
                request,
                response,
                signatures,
            } => {
                if let RequestSate::Evaluation = self.state.clone() {
                } else {
                    todo!()
                };

                if response.appr_required {
                    if let Err(e) = self
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
                            Some(response.eval_success),
                            response.appr_required,
                            None,
                            Some(HashSet::from_iter(
                                signatures.iter().cloned(),
                            )),
                            None,
                        )
                        .await
                    {
                        Ok(data) => data,
                        Err(e) => todo!(),
                    };

                    if let Err(e) = self.validation(ctx, data).await {
                        todo!()
                    }
                }
            }
            RequestManagerMessage::FinishRequest => {
                if let RequestSate::Distribution { .. } = self.state.clone() {
                } else {
                    todo!()
                };

                if let Err(e) = self.end_request(ctx).await {
                    todo!()
                }

                // TODO Limpiar la base de datos.
                ctx.stop().await;
            }
            RequestManagerMessage::ValidationRes {
                result,
                signatures,
                errors,
            } => {
                let actual_state = if let RequestSate::Validation(state) =
                    self.state.clone()
                {
                    state
                } else {
                    todo!()
                };

                let (ledger, event) = self.create_ledger_event(
                    actual_state,
                    signatures,
                    result,
                    &errors,
                );
                if let Err(e) = self.safe_ledger_event(ctx, event, ledger).await
                {
                    todo!()
                }
            }
            RequestManagerMessage::Fact => {
                if let RequestSate::Starting = self.state {
                } else {
                    todo!()
                };

                if let Err(e) = self.evaluation(ctx).await {
                    todo!()
                };
            }
            RequestManagerMessage::Other => {
                if let RequestSate::Starting = self.state {
                } else {
                    todo!()
                };

                let data = match self
                    .build_data_event_proof(
                        ctx,
                        None,
                        LedgerValue::Patch(ValueWrapper(
                            serde_json::Value::String("[]".to_owned()),
                        )),
                        None,
                        None,
                        false,
                        None,
                        None,
                        None,
                    )
                    .await
                {
                    Ok(data) => data,
                    Err(e) => todo!(),
                };

                if let Err(e) = self.validation(ctx, data).await {
                    todo!()
                };
            }
        }

        Ok(RequestManagerResponse::None)
    }

    async fn on_event(
        &mut self,
        event: RequestManagerEvent,
        ctx: &mut ActorContext<RequestManager>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}

#[async_trait]
impl Storable for RequestManager {}
