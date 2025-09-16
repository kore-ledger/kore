// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Evaluation module.
//! This module contains the evaluation logic for the Kore protocol.
//!

pub mod compiler;
pub mod evaluator;
pub mod request;
pub mod response;
mod runner;
pub mod schema;

use crate::{
    DIGEST_DERIVATOR,
    auth::WitnessesAuth,
    governance::{Governance, Quorum, model::ProtocolTypes},
    model::{
        HashId, SignTypesNode, ValueWrapper,
        common::{
            emit_fail, get_metadata, get_sign, get_signers_quorum_gov_version,
            send_reboot_to_req, take_random_signers, try_to_update,
        },
        event::{LedgerValue, ProtocolsError, ProtocolsSignatures},
        request::EventRequest,
        signature::{Signature, Signed},
    },
    request::manager::{RequestManager, RequestManagerMessage},
    subject::Metadata,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    Event, Handler, Message,
};

use async_trait::async_trait;
use evaluator::{Evaluator, EvaluatorMessage};
use identity::identifier::{KeyIdentifier, derive::digest::DigestDerivator};
use request::{EvaluationReq, SubjectContext};
use response::{EvalLedgerResponse, EvaluationRes, Response as EvalRes};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, warn};

const TARGET_EVALUATION: &str = "Kore-Evaluation";

use std::collections::HashSet;
// TODO cuando se recibe una evaluación, validación lo que sea debería venir firmado y comprobar que es de quien dice ser, cuando llega por la network y cuando la envía un usuario.
#[derive(Default)]
pub struct Evaluation {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // Actual responses
    evaluators_response: Vec<EvalRes>,
    // Evaluators quantity
    evaluators_quantity: u32,

    evaluators_signatures: Vec<ProtocolsSignatures>,

    request_id: String,

    version: u64,

    errors: String,

    signed_eval_req: Option<Signed<EvaluationReq>>,

    reboot: bool,

    current_evaluators: HashSet<KeyIdentifier>,

    pending_evaluators: HashSet<KeyIdentifier>,
}

impl Evaluation {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Evaluation {
            node_key,
            ..Default::default()
        }
    }

    async fn end_evaluators(
        &self,
        ctx: &mut ActorContext<Evaluation>,
    ) -> Result<(), ActorError> {
        for evaluator in self.current_evaluators.clone() {
            let child: Option<ActorRef<Evaluator>> =
                ctx.get_child(&evaluator.to_string()).await;
            if let Some(child) = child {
                child.ask_stop().await?;
            }
        }

        Ok(())
    }

    fn check_evaluator(&mut self, evaluator: KeyIdentifier) -> bool {
        self.current_evaluators.remove(&evaluator)
    }

    fn create_evaluation_req(
        &self,
        event_request: Signed<EventRequest>,
        metadata: Metadata,
        state: ValueWrapper,
        gov_state_init_state: ValueWrapper,
        gov_version: u64,
    ) -> EvaluationReq {
        EvaluationReq {
            event_request: event_request.clone(),
            context: SubjectContext {
                subject_id: metadata.subject_id,
                governance_id: metadata.governance_id,
                schema_id: metadata.schema_id,
                is_owner: self.node_key == event_request.signature.signer,
                namespace: metadata.namespace,
            },
            new_owner: metadata.new_owner,
            state,
            gov_state_init_state,
            sn: metadata.sn + 1,
            gov_version,
        }
    }

    async fn create_evaluators(
        &self,
        ctx: &mut ActorContext<Evaluation>,
        evaluation_req: Signed<EvaluationReq>,
        schema_id: &str,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        // Create Evaluator child
        let child = ctx
            .create_child(
                &format!("{}", signer),
                Evaluator::new(
                    self.request_id.to_string(),
                    self.version,
                    signer.clone(),
                ),
            )
            .await;
        let evaluator_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(e),
        };

        // Check node_key
        let our_key = self.node_key.clone();
        // We are signer
        if signer == our_key {
            evaluator_actor
                .tell(EvaluatorMessage::LocalEvaluation {
                    evaluation_req: evaluation_req.content,
                    our_key: signer,
                })
                .await?
        }
        // Other node is signer
        else {
            evaluator_actor
                .tell(EvaluatorMessage::NetworkEvaluation {
                    evaluation_req,
                    node_key: signer,
                    our_key,
                    schema_id: schema_id.to_owned(),
                })
                .await?
        }

        Ok(())
    }

    fn check_responses(&self) -> bool {
        let set: HashSet<EvalRes> =
            HashSet::from_iter(self.evaluators_response.iter().cloned());

        set.len() == 1
    }

    async fn fail_evaluation(
        &self,
        ctx: &mut ActorContext<Evaluation>,
    ) -> Result<EvalLedgerResponse, ActorError> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!(TARGET_EVALUATION, "Error getting derivator");
            DigestDerivator::Blake3_256
        };
        let (state, gov_id) = if let Some(req) = self.signed_eval_req.clone() {
            let gov_id = if req.content.context.governance_id.is_empty() {
                req.content.context.subject_id
            } else {
                req.content.context.governance_id
            };
            (req.content.state, gov_id)
        } else {
            return Err(ActorError::FunctionalFail(
                "Can not get eval request".to_owned(),
            ));
        };

        let state_hash = match state.hash_id(derivator) {
            Ok(state_hash) => state_hash,
            Err(e) => {
                return Err(ActorError::FunctionalFail(format!(
                    "Can not obtaing state hash: {}",
                    e
                )));
            }
        };

        let mut error = self.errors.clone();
        if self.errors.is_empty() {
            "who: ALL, error: No evaluator was able to evaluate the event."
                .clone_into(&mut error);

            let all_time_out = self
                .evaluators_signatures
                .iter()
                .all(|x| matches!(x, ProtocolsSignatures::TimeOut(_)));

            if all_time_out {
                try_to_update(ctx, gov_id, WitnessesAuth::Witnesses).await?
            }
        }

        Ok(EvalLedgerResponse {
            value: LedgerValue::Error(ProtocolsError {
                evaluation: Some(error),
                validation: None,
            }),
            state_hash,
            eval_success: false,
            appr_required: false,
        })
    }

    async fn send_evaluation_to_req(
        &self,
        ctx: &mut ActorContext<Evaluation>,
        response: EvalLedgerResponse,
    ) -> Result<(), ActorError> {
        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        let request = if let Some(req) = self.signed_eval_req.clone() {
            req.content
        } else {
            return Err(ActorError::FunctionalFail(
                "Can not get eval request".to_owned(),
            ));
        };

        if let Some(req_actor) = req_actor {
            req_actor
                .tell(RequestManagerMessage::EvaluationRes {
                    request: Box::new(request),
                    response,
                    signatures: self.evaluators_signatures.clone(),
                })
                .await?;
        } else {
            return Err(ActorError::NotFound(req_path));
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum EvaluationMessage {
    Create {
        request_id: String,
        version: u64,
        request: Signed<EventRequest>,
    },

    Response {
        evaluation_res: EvaluationRes,
        sender: KeyIdentifier,
        signature: Option<Signature>,
    },
}

impl Message for EvaluationMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationEvent {}

impl Event for EvaluationEvent {}

#[async_trait]
impl Actor for Evaluation {
    type Event = EvaluationEvent;
    type Message = EvaluationMessage;
    type Response = ();
}

#[async_trait]
impl Handler<Evaluation> for Evaluation {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: EvaluationMessage,
        ctx: &mut ActorContext<Evaluation>,
    ) -> Result<(), ActorError> {
        match msg {
            EvaluationMessage::Create {
                request_id,
                version,
                request,
            } => {
                let (subject_id, confirm) = match request.content.clone() {
                    EventRequest::Fact(event) => (event.subject_id, false),
                    EventRequest::Transfer(event) => (event.subject_id, false),
                    EventRequest::Confirm(event) => (event.subject_id, true),
                    _ => {
                        let e = "Only can evaluate Fact, Transfer and Confirm request";
                        error!(TARGET_EVALUATION, "Create, {}", e);

                        return Err(emit_fail(
                            ctx,
                            ActorError::FunctionalFail(e.to_owned()),
                        )
                        .await);
                    }
                };

                let metadata =
                    match get_metadata(ctx, &subject_id.to_string()).await {
                        Ok(metadata) => metadata,
                        Err(e) => {
                            error!(
                                TARGET_EVALUATION,
                                "Create, can not get metadata: {}", e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    };

                if confirm && !metadata.governance_id.is_empty() {
                    let e = "Confirm event in trazability subjects can not evaluate";
                    error!(TARGET_EVALUATION, "Create, {}", e);

                    return Err(emit_fail(
                        ctx,
                        ActorError::FunctionalFail(e.to_owned()),
                    )
                    .await);
                }

                let governance = if metadata.governance_id.is_empty() {
                    metadata.subject_id.clone()
                } else {
                    metadata.governance_id.clone()
                };

                let (state, gov_state_init_state) =
                    if let EventRequest::Transfer(_) = request.content.clone() {
                        if metadata.governance_id.is_empty() {
                            (
                                metadata.properties.clone(),
                                ValueWrapper(json!({})),
                            )
                        } else {
                            let metadata_gov = match get_metadata(
                                ctx,
                                &metadata.governance_id.to_string(),
                            )
                            .await
                            {
                                Ok(metadata) => metadata,
                                Err(e) => {
                                    error!(
                                        TARGET_EVALUATION,
                                        "Create, can not get metadata: {}", e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                }
                            };

                            (
                                metadata.properties.clone(),
                                metadata_gov.properties,
                            )
                        }
                    } else if let EventRequest::Fact(_) =
                        request.content.clone()
                    {
                        if metadata.governance_id.is_empty() {
                            (
                                metadata.properties.clone(),
                                ValueWrapper(json!({})),
                            )
                        } else {
                            let metadata_gov = match get_metadata(
                                ctx,
                                &metadata.governance_id.to_string(),
                            )
                            .await
                            {
                                Ok(metadata) => metadata,
                                Err(e) => {
                                    error!(
                                        TARGET_EVALUATION,
                                        "Create, can not get metadata: {}", e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                }
                            };

                            let governance = match Governance::try_from(
                                metadata_gov.properties.clone(),
                            ) {
                                Ok(gov) => gov,
                                Err(e) => {
                                    let e = format!(
                                        "can not convert governance from properties: {}",
                                        e
                                    );
                                    error!(TARGET_EVALUATION, "Create, {}", e);
                                    return Err(emit_fail(
                                        ctx,
                                        ActorError::FunctionalFail(e),
                                    )
                                    .await);
                                }
                            };

                            let init_value = match governance
                                .get_init_state(&metadata.schema_id)
                            {
                                Ok(init_value) => init_value,
                                Err(e) => {
                                    let e = format!(
                                        "can not obtain schema {} from governance: {}",
                                        metadata.schema_id, e
                                    );
                                    error!(TARGET_EVALUATION, "Create, {}", e);
                                    return Err(emit_fail(
                                        ctx,
                                        ActorError::FunctionalFail(e),
                                    )
                                    .await);
                                }
                            };

                            (metadata.properties.clone(), init_value)
                        }
                    } else {
                        (metadata.properties.clone(), ValueWrapper(json!({})))
                    };

                let (signers, quorum, gov_version) =
                    match get_signers_quorum_gov_version(
                        ctx,
                        &governance.to_string(),
                        &metadata.schema_id,
                        metadata.namespace.clone(),
                        ProtocolTypes::Evaluation,
                    )
                    .await
                    {
                        Ok(data) => data,
                        Err(e) => {
                            error!(
                                TARGET_EVALUATION,
                                "Create, can not get signers quorum and gov version: {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    };

                let eval_req = self.create_evaluation_req(
                    request,
                    metadata.clone(),
                    state,
                    gov_state_init_state,
                    gov_version,
                );

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::EvaluationReq(eval_req.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        error!(
                            TARGET_EVALUATION,
                            "Create, can not sign eval request: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let signed_evaluation_req: Signed<EvaluationReq> = Signed {
                    content: eval_req,
                    signature,
                };

                self.evaluators_response = vec![];
                self.signed_eval_req = Some(signed_evaluation_req.clone());
                self.quorum = quorum;
                self.evaluators_quantity = signers.len() as u32;
                self.request_id = request_id.to_string();
                self.version = version;
                self.evaluators_signatures = vec![];
                self.errors = String::default();
                self.reboot = false;

                if signers.is_empty() {
                    warn!(
                        TARGET_EVALUATION,
                        "Create, There are no evaluators available for the {} scheme",
                        metadata.schema_id
                    );

                    let response = match self.fail_evaluation(ctx).await {
                        Ok(res) => res,
                        Err(e) => {
                            error!(
                                TARGET_EVALUATION,
                                "Create, can not create evaluation response: {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    };
                    if let Err(e) =
                        self.send_evaluation_to_req(ctx, response).await
                    {
                        error!(
                            TARGET_EVALUATION,
                            "Create, can send evaluation to request actor: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    };
                }

                let evaluators_quantity = self.quorum.get_signers(
                    self.evaluators_quantity,
                    signers.len() as u32,
                );

                let (current_eval, pending_eval) =
                    take_random_signers(signers, evaluators_quantity as usize);
                self.current_evaluators.clone_from(&current_eval);
                self.pending_evaluators.clone_from(&pending_eval);

                for signer in current_eval {
                    if let Err(e) = self
                        .create_evaluators(
                            ctx,
                            signed_evaluation_req.clone(),
                            &metadata.schema_id,
                            signer.clone(),
                        )
                        .await
                    {
                        error!(
                            TARGET_EVALUATION,
                            "Can not create evaluator {}: {}", signer, e
                        );
                    }
                }
            }
            EvaluationMessage::Response {
                evaluation_res,
                sender,
                signature,
            } => {
                if !self.reboot {
                    // If node is in evaluator list
                    if self.check_evaluator(sender.clone()) {
                        // Check type of validation
                        match evaluation_res {
                            EvaluationRes::Response(response) => {
                                if let Some(signature) = signature {
                                    self.evaluators_signatures.push(
                                        ProtocolsSignatures::Signature(
                                            signature,
                                        ),
                                    );
                                } else {
                                    let e =
                                        "Evaluation solver whitout signature"
                                            .to_owned();
                                    error!(
                                        TARGET_EVALUATION,
                                        "Response, {}", e
                                    );
                                    return Err(ActorError::Functional(e));
                                }

                                self.evaluators_response.push(response);
                            }
                            EvaluationRes::TimeOut(timeout) => self
                                .evaluators_signatures
                                .push(ProtocolsSignatures::TimeOut(timeout)),
                            EvaluationRes::Error(error) => {
                                self.errors = format!(
                                    "{} who: {}, error: {}.",
                                    self.errors, sender, error
                                );
                            }
                            EvaluationRes::Reboot => {
                                let governance_id = if let Some(req) =
                                    self.signed_eval_req.clone()
                                {
                                    req.content.context.governance_id
                                } else {
                                    let e = ActorError::FunctionalFail(
                                        "Can not get eval request".to_owned(),
                                    );
                                    error!(
                                        TARGET_EVALUATION,
                                        "Response, can not get eval request: {}",
                                        e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                };

                                if let Err(e) = send_reboot_to_req(
                                    ctx,
                                    &self.request_id,
                                    governance_id,
                                )
                                .await
                                {
                                    error!(
                                        TARGET_EVALUATION,
                                        "Response, can not send reboot to Request actor: {}",
                                        e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                }
                                self.reboot = true;

                                if let Err(e) = self.end_evaluators(ctx).await {
                                    error!(
                                        TARGET_EVALUATION,
                                        "Response, can not end evaluators: {}",
                                        e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                };

                                return Ok(());
                            }
                        };

                        if self.quorum.check_quorum(
                            self.evaluators_quantity,
                            self.evaluators_response.len() as u32,
                        ) {
                            let response = if self.check_responses() {
                                EvalLedgerResponse::from(
                                    self.evaluators_response[0].clone(),
                                )
                            } else {
                                self.errors = format!(
                                    "{} who: ALL, error: Several evaluations were correct, but there are some different.",
                                    self.errors
                                );
                                match self.fail_evaluation(ctx).await {
                                    Ok(res) => res,
                                    Err(e) => {
                                        error!(
                                            TARGET_EVALUATION,
                                            "Response, can not create evaluation response: {}",
                                            e
                                        );
                                        return Err(emit_fail(ctx, e).await);
                                    }
                                }
                            };

                            if let Err(e) =
                                self.send_evaluation_to_req(ctx, response).await
                            {
                                error!(
                                    TARGET_EVALUATION,
                                    "Response, can send evaluation to request actor: {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            };
                        } else if self.current_evaluators.is_empty()
                            && !self.pending_evaluators.is_empty()
                        {
                            if let Some(req) = self.signed_eval_req.clone() {
                                let evaluators_quantity =
                                    self.quorum.get_signers(
                                        self.evaluators_quantity,
                                        self.pending_evaluators.len() as u32,
                                    );

                                let (current_eval, pending_eval) =
                                    take_random_signers(
                                        self.pending_evaluators.clone(),
                                        evaluators_quantity as usize,
                                    );
                                self.current_evaluators
                                    .clone_from(&current_eval);
                                self.pending_evaluators
                                    .clone_from(&pending_eval);

                                for signer in current_eval {
                                    if let Err(e) = self
                                        .create_evaluators(
                                            ctx,
                                            req.clone(),
                                            &req.content.context.schema_id,
                                            signer.clone(),
                                        )
                                        .await
                                    {
                                        error!(
                                            TARGET_EVALUATION,
                                            "Can not create evaluator {}: {}",
                                            signer,
                                            e
                                        );
                                    }
                                }
                            } else {
                                let e = ActorError::FunctionalFail(
                                    "Can not get evaluation request".to_owned(),
                                );
                                error!(
                                    TARGET_EVALUATION,
                                    "Response, can not get validation request: {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            };
                        } else if self.current_evaluators.is_empty() {
                            let response = match self.fail_evaluation(ctx).await
                            {
                                Ok(res) => res,
                                Err(e) => {
                                    error!(
                                        TARGET_EVALUATION,
                                        "Response, can not create evaluation response: {}",
                                        e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                }
                            };
                            if let Err(e) =
                                self.send_evaluation_to_req(ctx, response).await
                            {
                                error!(
                                    TARGET_EVALUATION,
                                    "Response, can send evaluation to request actor: {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            };
                        }
                    } else {
                        warn!(
                            TARGET_EVALUATION,
                            "Response, A response has been received from someone we were not expecting."
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Evaluation>,
    ) -> ChildAction {
        error!(TARGET_EVALUATION, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::Duration};

    use actor::{ActorPath, ActorRef, SystemRef};
    use identity::{
        identifier::{DigestIdentifier, derive::digest::DigestDerivator},
        keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
    };
    use serde_json::json;
    use test_log::test;

    use crate::{
        EventRequest, FactRequest, Governance, NodeMessage, NodeResponse,
        Signed, SubjectMessage, SubjectResponse, ValueWrapper,
        approval::approver::ApprovalStateRes,
        model::{
            HashId, Namespace, SignTypesNode, event::LedgerValue,
            request::TransferRequest,
        },
        node::Node,
        query::{Query, QueryMessage, QueryResponse},
        request::{
            RequestHandler, RequestHandlerMessage, RequestHandlerResponse,
        },
        subject::{
            Subject,
            event::{LedgerEvent, LedgerEventMessage, LedgerEventResponse},
        },
        validation::tests::create_subject_gov,
    };

    #[test(tokio::test)]
    async fn test_fact_gov() {
        let (
            _system,
            node_actor,
            request_actor,
            query_actor,
            subject_actor,
            ledger_event_actor,
            subject_id,
        ) = create_subject_gov().await;

        let fact_request = EventRequest::Fact(FactRequest {
            subject_id: subject_id.clone(),
            payload: ValueWrapper(json!({
                "members": {
                    "add": [
                        {
                            "name": "KoreNode1",
                            "key": "EUrVnqpwo9EKBvMru4wWLMpJgOTKM5gZnxApRmjrRbbE"
                        }
                    ]
                }
            })),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                fact_request.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: fact_request,
            signature,
        };

        let RequestHandlerResponse::Ok(request_id) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(1)).await;

        let QueryResponse::RequestState(state) = query_actor
            .ask(QueryMessage::GetRequestState {
                request_id: request_id.request_id.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!("In Approval", state.status);
        let QueryResponse::ApprovalState(data) = query_actor
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(data.state, "Pending");

        let RequestHandlerResponse::Response(res) = request_actor
            .ask(RequestHandlerMessage::ChangeApprovalState {
                subject_id: subject_id.to_string(),
                state: ApprovalStateRes::RespondedAccepted,
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(
            res,
            format!(
                "The approval request for subject {} has changed to RespondedAccepted",
                subject_id.to_string()
            )
        );

        tokio::time::sleep(Duration::from_secs(1)).await;
        let QueryResponse::ApprovalState(data) = query_actor
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(data.state, "RespondedAccepted");

        let LedgerEventResponse::LastEvent(last_event) = ledger_event_actor
            .ask(LedgerEventMessage::GetLastEvent)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        let SubjectResponse::Metadata(metadata) = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(last_event.content.subject_id, subject_id);
        assert_eq!(last_event.content.event_request, signed_event_req);
        assert_eq!(last_event.content.sn, 1);
        assert_eq!(last_event.content.gov_version, 0);
        assert_eq!(
            last_event.content.value,
            LedgerValue::Patch(ValueWrapper(json!([
                {"op":"add","path":"/members/KoreNode1","value":"EUrVnqpwo9EKBvMru4wWLMpJgOTKM5gZnxApRmjrRbbE"}])))
        );
        assert!(last_event.content.eval_success.unwrap());
        assert!(last_event.content.appr_required);
        assert!(last_event.content.appr_success.unwrap());
        assert!(last_event.content.vali_success);
        assert!(!last_event.content.evaluators.unwrap().is_empty());
        assert!(!last_event.content.approvers.unwrap().is_empty());
        assert!(!last_event.content.validators.is_empty());

        assert_eq!(metadata.subject_id, subject_id);
        assert_eq!(metadata.governance_id.to_string(), "");
        assert_eq!(metadata.name.unwrap(), "Name");
        assert_eq!(metadata.description.unwrap(), "Description");
        assert_eq!(metadata.genesis_gov_version, 0);
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 1);
        assert!(metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 1);
        // TODO MEJORAR
        assert!(!gov.members.is_empty());
        assert!(gov.roles_schema.is_empty());
        assert!(gov.schemas.is_empty());
        assert!(gov.policies_schema.is_empty());
    }

    #[test(tokio::test)]
    async fn test_transfer_req() {
        let (
            _system,
            node_actor,
            request_actor,
            _query_actor,
            subject_actor,
            ledger_event_actor,
            subject_id,
        ) = create_subject_gov().await;

        let new_owner = KeyPair::Ed25519(Ed25519KeyPair::new());

        let fact_request = EventRequest::Fact(FactRequest {
            subject_id: subject_id.clone(),
            payload: ValueWrapper(json!({
                "members": {
                    "add": [
                        {
                            "name": "TestMember",
                            "key": new_owner.key_identifier().to_string()
                        }
                    ]
                }
            })),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                fact_request.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: fact_request,
            signature,
        };

        let RequestHandlerResponse::Ok(_request_id) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(9)).await;

        let RequestHandlerResponse::Response(res) = request_actor
            .ask(RequestHandlerMessage::ChangeApprovalState {
                subject_id: subject_id.to_string(),
                state: ApprovalStateRes::RespondedAccepted,
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(
            res,
            format!(
                "The approval request for subject {} has changed to RespondedAccepted",
                subject_id.to_string()
            )
        );

        let transfer_reques = EventRequest::Transfer(TransferRequest {
            subject_id: subject_id.clone(),
            new_owner: new_owner.key_identifier().clone(),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                transfer_reques.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: transfer_reques,
            signature,
        };

        let RequestHandlerResponse::Ok(_response) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(10)).await;

        let LedgerEventResponse::LastEvent(last_event) = ledger_event_actor
            .ask(LedgerEventMessage::GetLastEvent)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        let SubjectResponse::Metadata(metadata) = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(last_event.content.subject_id, subject_id);
        assert_eq!(last_event.content.event_request, signed_event_req);
        assert_eq!(last_event.content.sn, 2);
        assert_eq!(last_event.content.gov_version, 1);
        assert_eq!(
            last_event.content.value,
            LedgerValue::Patch(ValueWrapper(serde_json::Value::String(
                "[]".to_owned(),
            ),))
        );
        assert!(last_event.content.eval_success.unwrap());
        assert!(!last_event.content.appr_required);
        assert!(last_event.content.appr_success.is_none());
        assert!(last_event.content.vali_success);
        assert!(!last_event.content.evaluators.unwrap().is_empty());
        assert!(last_event.content.approvers.is_none(),);
        assert!(!last_event.content.validators.is_empty());

        assert_eq!(metadata.subject_id, subject_id);
        assert_eq!(metadata.governance_id.to_string(), "");
        assert_eq!(metadata.genesis_gov_version, 0);
        assert_eq!(metadata.name.unwrap(), "Name");
        assert_eq!(metadata.description.unwrap(), "Description");
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 2);
        assert_eq!(metadata.new_owner.unwrap(), new_owner.key_identifier());
        assert!(metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 2);
        // TODO MEJORAR
        assert!(!gov.members.is_empty());
        assert!(gov.roles_schema.is_empty());
        assert!(gov.schemas.is_empty());
        assert!(gov.policies_schema.is_empty());

        if !request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .is_err()
        {
            panic!("Invalid response")
        }
    }

    async fn init_gov_sub() -> (
        SystemRef,
        ActorRef<Node>,
        ActorRef<RequestHandler>,
        ActorRef<Query>,
        ActorRef<Subject>,
        ActorRef<LedgerEvent>,
        DigestIdentifier,
    ) {
        let (
            system,
            node_actor,
            request_actor,
            query_actor,
            subject_actor,
            ledger_event_actor,
            subject_id,
        ) = create_subject_gov().await;

        let fact_request = EventRequest::Fact(FactRequest {
            subject_id: subject_id.clone(),
            payload: ValueWrapper(json!({
                "schemas": {
                    "add": [
                        {
                            "id": "Example",
                            "contract": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=",
                            "initial_value": {
                                "one": 0,
                                "two": 0,
                                "three": 0
                            }
                        }
                    ]
                },
                "roles": {
                    "all_schemas": {
                        "add": {
                            "evaluator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "validator": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                            "witness": [
                                {
                                    "name": "Owner",
                                    "namespace": []
                                }
                            ],
                        }
                    },
                    "schema":
                        [
                        {
                            "schema_id": "Example",
                            "roles": {
                                "add": {
                                    "creator": [
                                        {
                                            "name": "Owner",
                                            "namespace": [],
                                            "quantity": 2
                                        }
                                    ],
                                    "issuer": [
                                        {
                                            "name": "Owner",
                                            "namespace": [],
                                        }
                                    ]
                                }
                            }
                        }
                    ]
                }
            })),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                fact_request.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: fact_request,
            signature,
        };

        let RequestHandlerResponse::Ok(request_id) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(9)).await;

        let QueryResponse::RequestState(_state) = query_actor
            .ask(QueryMessage::GetRequestState {
                request_id: request_id.request_id.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(3)).await;

        let QueryResponse::ApprovalState(data) = query_actor
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(data.state, "Pending");

        let RequestHandlerResponse::Response(res) = request_actor
            .ask(RequestHandlerMessage::ChangeApprovalState {
                subject_id: subject_id.to_string(),
                state: ApprovalStateRes::RespondedAccepted,
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(
            res,
            format!(
                "The approval request for subject {} has changed to RespondedAccepted",
                subject_id.to_string()
            )
        );

        tokio::time::sleep(Duration::from_secs(1)).await;
        let QueryResponse::ApprovalState(data) = query_actor
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(data.state, "RespondedAccepted");

        let LedgerEventResponse::LastEvent(last_event) = ledger_event_actor
            .ask(LedgerEventMessage::GetLastEvent)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        let SubjectResponse::Metadata(metadata) = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(last_event.content.subject_id, subject_id);
        assert_eq!(last_event.content.event_request, signed_event_req);
        assert_eq!(last_event.content.sn, 1);
        assert_eq!(last_event.content.gov_version, 0);

        assert_eq!(
            last_event.content.value,
            LedgerValue::Patch(ValueWrapper(
                json!([{"op":"add","path":"/policies_schema/Example","value":{"evaluate":"majority","validate":"majority"}},{"op":"add","path":"/roles_all_schemas/evaluator/0","value":{"name":"Owner","namespace":[]}},{"op":"add","path":"/roles_all_schemas/validator/0","value":{"name":"Owner","namespace":[]}},{"op":"add","path":"/roles_all_schemas/witness/0","value":{"name":"Owner","namespace":[]}},{"op":"add","path":"/roles_schema/Example","value":{"creator":[{"name":"Owner","namespace":[],"quantity":2,"witnesses":["Witnesses"]}],"evaluator":[],"issuer":{"any":false,"users":[{"name":"Owner","namespace":[]}]},"validator":[],"witness":[]}},{"op":"add","path":"/schemas/Example","value":{"contract":"dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07CnVzZSBrb3JlX2NvbnRyYWN0X3NkayBhcyBzZGs7CgovLy8gRGVmaW5lIHRoZSBzdGF0ZSBvZiB0aGUgY29udHJhY3QuIAojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCnN0cnVjdCBTdGF0ZSB7CiAgcHViIG9uZTogdTMyLAogIHB1YiB0d286IHUzMiwKICBwdWIgdGhyZWU6IHUzMgp9CgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplKV0KZW51bSBTdGF0ZUV2ZW50IHsKICBNb2RPbmUgeyBkYXRhOiB1MzIgfSwKICBNb2RUd28geyBkYXRhOiB1MzIgfSwKICBNb2RUaHJlZSB7IGRhdGE6IHUzMiB9LAogIE1vZEFsbCB7IG9uZTogdTMyLCB0d286IHUzMiwgdGhyZWU6IHUzMiB9Cn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gbWFpbl9mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMiwgaW5pdF9zdGF0ZV9wdHI6IGkzMiwgZXZlbnRfcHRyOiBpMzIsIGlzX293bmVyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpleGVjdXRlX2NvbnRyYWN0KHN0YXRlX3B0ciwgaW5pdF9zdGF0ZV9wdHIsIGV2ZW50X3B0ciwgaXNfb3duZXIsIGNvbnRyYWN0X2xvZ2ljKQp9CgojW3Vuc2FmZShub19tYW5nbGUpXQpwdWIgdW5zYWZlIGZuIGluaXRfY2hlY2tfZnVuY3Rpb24oc3RhdGVfcHRyOiBpMzIpIC0+IHUzMiB7CiAgc2RrOjpjaGVja19pbml0X2RhdGEoc3RhdGVfcHRyLCBpbml0X2xvZ2ljKQp9CgpmbiBpbml0X2xvZ2ljKAogIF9zdGF0ZTogJlN0YXRlLAogIGNvbnRyYWN0X3Jlc3VsdDogJm11dCBzZGs6OkNvbnRyYWN0SW5pdENoZWNrLAopIHsKICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0KCmZuIGNvbnRyYWN0X2xvZ2ljKAogIGNvbnRleHQ6ICZzZGs6OkNvbnRleHQ8U3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBpZiBkYXRhID09IDUwIHsKICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDYW4gbm90IGNoYW5nZSB0aHJlZSB2YWx1ZSwgNTAgaXMgYSBpbnZhbGlkIHZhbHVlIi50b19vd25lZCgpOwogICAgICAgICAgcmV0dXJuCiAgICAgICAgfQogICAgICAgIAogICAgICAgIHN0YXRlLnRocmVlID0gZGF0YTsKICAgICAgfSwKICAgICAgU3RhdGVFdmVudDo6TW9kQWxsIHsgb25lLCB0d28sIHRocmVlIH0gPT4gewogICAgICAgIHN0YXRlLm9uZSA9IG9uZTsKICAgICAgICBzdGF0ZS50d28gPSB0d287CiAgICAgICAgc3RhdGUudGhyZWUgPSB0aHJlZTsKICAgICAgfQogIH0KICBjb250cmFjdF9yZXN1bHQuc3VjY2VzcyA9IHRydWU7Cn0=","initial_value":{"one":0,"three":0,"two":0}}}])
            ))
        );
        assert!(last_event.content.eval_success.unwrap());
        assert!(last_event.content.appr_required);
        assert!(last_event.content.appr_success.unwrap());
        assert!(last_event.content.vali_success);
        assert!(!last_event.content.evaluators.unwrap().is_empty());
        assert!(!last_event.content.approvers.unwrap().is_empty());
        assert!(!last_event.content.validators.is_empty());

        assert_eq!(metadata.subject_id, subject_id);
        assert_eq!(metadata.governance_id.to_string(), "");
        assert_eq!(metadata.genesis_gov_version, 0);
        assert_eq!(metadata.name.unwrap(), "Name");
        assert_eq!(metadata.description.unwrap(), "Description");
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 1);
        assert!(metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 1);
        // TODO MEJORAR
        assert!(!gov.members.is_empty());
        assert!(!gov.roles_schema.is_empty());
        assert!(!gov.schemas.is_empty());
        assert!(!gov.policies_schema.is_empty());

        (
            system,
            node_actor,
            request_actor,
            query_actor,
            subject_actor,
            ledger_event_actor,
            subject_id,
        )
    }

    #[test(tokio::test)]
    async fn test_fact_sub() {
        init_gov_sub().await;
    }

    async fn create_subject() -> (
        SystemRef,
        ActorRef<Node>,
        ActorRef<RequestHandler>,
        ActorRef<Query>,
        ActorRef<Subject>,
        ActorRef<LedgerEvent>,
        DigestIdentifier,
    ) {
        let (
            system,
            node_actor,
            request_actor,
            query_actor,
            _subject_actor,
            _ledger_event_actor,
            gov_id,
        ) = init_gov_sub().await;

        let create_request = EventRequest::Create(crate::CreateRequest {
            name: Some("Subject Name".to_owned()),
            description: Some("Subject Description".to_owned()),
            governance_id: gov_id.clone(),
            schema_id: "Example".to_owned(),
            namespace: Namespace::new(),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                create_request.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: create_request,
            signature,
        };

        let RequestHandlerResponse::Ok(request_id) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(1)).await;

        let QueryResponse::RequestState(state) = query_actor
            .ask(QueryMessage::GetRequestState {
                request_id: request_id.request_id.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!("Finish", state.status);

        let ledger_event_actor: ActorRef<LedgerEvent> = system
            .get_actor(&ActorPath::from(format!(
                "/user/node/{}/ledger_event",
                request_id.subject_id
            )))
            .await
            .unwrap();

        let LedgerEventResponse::LastEvent(last_event) = ledger_event_actor
            .ask(LedgerEventMessage::GetLastEvent)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        let subject_actor: ActorRef<Subject> = system
            .get_actor(&ActorPath::from(format!(
                "/user/node/{}",
                request_id.subject_id
            )))
            .await
            .unwrap();

        let SubjectResponse::Metadata(metadata) = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(
            last_event.content.subject_id.to_string(),
            request_id.subject_id
        );
        assert_eq!(last_event.content.event_request, signed_event_req);
        assert_eq!(last_event.content.sn, 0);
        assert_eq!(last_event.content.gov_version, 1);
        assert_eq!(
            last_event.content.value,
            LedgerValue::Patch(ValueWrapper(json!({
                "one": 0, "three": 0, "two": 0
            })))
        );
        assert_eq!(
            last_event.content.state_hash,
            metadata
                .properties
                .hash_id(DigestDerivator::Blake3_256)
                .unwrap()
        );
        assert!(last_event.content.eval_success.is_none());
        assert!(!last_event.content.appr_required);
        assert!(last_event.content.appr_success.is_none());
        assert!(last_event.content.vali_success);
        assert_eq!(
            last_event.content.hash_prev_event,
            DigestIdentifier::default()
        );
        assert!(last_event.content.evaluators.is_none());
        assert!(last_event.content.approvers.is_none(),);
        assert!(!last_event.content.validators.is_empty());

        assert_eq!(metadata.subject_id.to_string(), request_id.subject_id);
        assert_eq!(metadata.governance_id.to_string(), gov_id.to_string());
        assert_eq!(metadata.name.unwrap(), "Subject Name");
        assert_eq!(metadata.description.unwrap(), "Subject Description");
        assert_eq!(metadata.genesis_gov_version, 1);
        assert_eq!(metadata.schema_id, "Example");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 0);
        assert!(metadata.active);

        (
            system,
            node_actor,
            request_actor,
            query_actor,
            subject_actor,
            ledger_event_actor,
            DigestIdentifier::from_str(&request_id.subject_id).unwrap(),
        )
    }

    #[test(tokio::test)]
    async fn test_create_subject() {
        let _ = create_subject().await;
    }

    #[test(tokio::test)]
    async fn test_subject_events() {
        let (
            _system,
            node_actor,
            request_actor,
            _query_actor,
            subject_actor,
            ledger_event_actor,
            subject_id,
        ) = create_subject().await;

        let fact_request = EventRequest::Fact(crate::FactRequest {
            subject_id,
            payload: ValueWrapper(json!({
                "ModOne": {
                    "data": 100
                }
            })),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                fact_request.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: fact_request,
            signature,
        };

        let RequestHandlerResponse::Ok(request_id) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        tokio::time::sleep(Duration::from_secs(1)).await;
        let LedgerEventResponse::LastEvent(last_event) = ledger_event_actor
            .ask(LedgerEventMessage::GetLastEvent)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        let SubjectResponse::Metadata(metadata) = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(
            last_event.content.subject_id.to_string(),
            request_id.subject_id
        );
        assert_eq!(last_event.content.event_request, signed_event_req);
        assert_eq!(last_event.content.sn, 1);
        assert_eq!(last_event.content.gov_version, 1);
        assert_eq!(
            last_event.content.state_hash,
            metadata
                .properties
                .hash_id(DigestDerivator::Blake3_256)
                .unwrap()
        );
        assert!(last_event.content.eval_success.unwrap());
        assert!(!last_event.content.appr_required);
        assert!(last_event.content.appr_success.is_none());
        assert!(last_event.content.vali_success);
        assert!(!last_event.content.evaluators.unwrap().is_empty());
        assert!(last_event.content.approvers.is_none(),);
        assert!(!last_event.content.validators.is_empty());

        assert_eq!(metadata.subject_id.to_string(), request_id.subject_id);
        assert_eq!(metadata.genesis_gov_version, 1);
        assert_eq!(metadata.name.unwrap(), "Subject Name");
        assert_eq!(metadata.description.unwrap(), "Subject Description");
        assert_eq!(metadata.schema_id, "Example");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 1);
        assert!(metadata.active);
        assert!(metadata.properties.0["one"].as_u64().unwrap() == 100);
        assert!(metadata.properties.0["two"].as_u64().unwrap() == 0);
        assert!(metadata.properties.0["three"].as_u64().unwrap() == 0);
    }
}
