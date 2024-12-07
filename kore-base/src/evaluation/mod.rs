// Copyright 2024 Kore Ledger, SL
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
    governance::{model::Roles, Quorum},
    model::{
        common::{
            emit_fail, get_metadata, get_sign, get_signers_quorum_gov_version,
            send_reboot_to_req, try_to_update,
        },
        event::{LedgerValue, ProtocolsError, ProtocolsSignatures},
        request::EventRequest,
        signature::{Signature, Signed},
        HashId, SignTypesNode,
    },
    request::manager::{RequestManager, RequestManagerMessage},
    subject::Metadata,
    DIGEST_DERIVATOR,
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    Event, Handler, Message,
};

use async_trait::async_trait;
use evaluator::{Evaluator, EvaluatorMessage};
use identity::identifier::{derive::digest::DigestDerivator, KeyIdentifier};
use request::{EvaluationReq, SubjectContext};
use response::{EvalLedgerResponse, EvaluationRes, Response as EvalRes};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

const TARGET_EVALUATION: &str = "Kore-Evaluation";

use std::collections::HashSet;
// TODO cuando se recibe una evaluación, validación lo que sea debería venir firmado y comprobar que es de quien dice ser, cuando llega por la network y cuando la envía un usuario.
#[derive(Default)]
pub struct Evaluation {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // Evaluators
    evaluators: HashSet<KeyIdentifier>,
    // Actual responses
    evaluators_response: Vec<EvalRes>,
    // Evaluators quantity
    evaluators_quantity: u32,

    evaluators_signatures: Vec<ProtocolsSignatures>,
    request_id: String,
    errors: String,

    eval_req: Option<EvaluationReq>,

    reboot: bool,
}

impl Evaluation {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Evaluation {
            node_key,
            ..Default::default()
        }
    }

    async fn end_evaluators(&self, ctx: &mut ActorContext<Evaluation>) {
        for evaluator in self.evaluators.clone() {
            let child: Option<ActorRef<Evaluator>> =
                ctx.get_child(&evaluator.to_string()).await;
            if let Some(child) = child {
                child.stop().await;
            }
        }
    }

    fn check_evaluator(&mut self, evaluator: KeyIdentifier) -> bool {
        self.evaluators.remove(&evaluator)
    }

    fn create_evaluation_req(
        &self,
        event_request: Signed<EventRequest>,
        metadata: Metadata,
        gov_version: u64,
    ) -> EvaluationReq {
        EvaluationReq {
            event_request: event_request.clone(),
            context: SubjectContext {
                subject_id: metadata.subject_id,
                governance_id: metadata.governance_id,
                schema_id: metadata.schema_id,
                is_owner: self.node_key == event_request.signature.signer,
                state: metadata.properties,
                namespace: metadata.namespace.to_string(),
            },
            sn: metadata.sn + 1,
            gov_version,
        }
    }

    async fn create_evaluators(
        &self,
        ctx: &mut ActorContext<Evaluation>,
        request_id: &str,
        evaluation_req: Signed<EvaluationReq>,
        schema: &str,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        // Create Evaluator child
        let child = ctx
            .create_child(
                &format!("{}", signer),
                Evaluator::new(request_id.to_string(), signer.clone()),
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
                    request_id: request_id.to_owned(),
                    evaluation_req,
                    node_key: signer,
                    our_key,
                    schema: schema.to_owned(),
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
        let (state, subject_id) = if let Some(req) = self.eval_req.clone() {
            (req.context.state, req.context.subject_id)
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
                )))
            }
        };

        let mut error = self.errors.clone();
        if self.errors.is_empty() {
            "who: ALL, error: No evaluator was able to evaluate the event."
                .clone_into(&mut error);
            try_to_update(self.evaluators_signatures.clone(), ctx, subject_id)
                .await?;
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

        let request = if let Some(req) = self.eval_req.clone() {
            req
        } else {
            return Err(ActorError::FunctionalFail(
                "Can not get eval request".to_owned(),
            ));
        };

        if let Some(req_actor) = req_actor {
            req_actor
                .tell(RequestManagerMessage::EvaluationRes {
                    request,
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

// TODO: revizar todos los errores, algunos pueden ser ActorError.
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
                request,
            } => {
                let subject_id = if let EventRequest::Fact(event) =
                    request.content.clone()
                {
                    event.subject_id
                } else {
                    error!(TARGET_EVALUATION, "Create, only can evaluate Fact request");
                    let e = ActorError::FunctionalFail(
                        "Only can eval Fact requests".to_owned(),
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                let metadata =
                    match get_metadata(ctx, &subject_id.to_string()).await {
                        Ok(metadata) => metadata,
                        Err(e) => {
                            error!(TARGET_EVALUATION, "Create, can not get metadata: {}", e);
                            return Err(emit_fail(ctx, e).await);
                        }
                    };

                let governance = if metadata.governance_id.is_empty() {
                    metadata.subject_id.clone()
                } else {
                    metadata.governance_id.clone()
                };

                let (signers, quorum, gov_version) =
                    match get_signers_quorum_gov_version(
                        ctx,
                        &governance.to_string(),
                        &metadata.schema_id,
                        metadata.namespace.clone(),
                        Roles::EVALUATOR,
                    )
                    .await
                    {
                        Ok(data) => data,
                        Err(e) => {
                            error!(TARGET_EVALUATION, "Create, can not get signersm quorum and gov version: {}", e);
                            return Err(emit_fail(ctx, e).await);
                        }
                    };

                let eval_req = self.create_evaluation_req(
                    request,
                    metadata.clone(),
                    gov_version,
                );

                self.evaluators_response = vec![];
                self.eval_req = Some(eval_req.clone());
                self.quorum = quorum;
                self.evaluators.clone_from(&signers);
                self.evaluators_quantity = signers.len() as u32;
                self.request_id = request_id.to_string();
                self.evaluators_signatures = vec![];
                self.errors = String::default();

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::EvaluationReq(eval_req.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        error!(TARGET_EVALUATION, "Create, can not sign eval request: {}", e);
                        return Err(emit_fail(ctx, e).await)
                    },
                };

                let signed_evaluation_req: Signed<EvaluationReq> = Signed {
                    content: eval_req,
                    signature,
                };

                for signer in signers {
                    self.create_evaluators(
                        ctx,
                        &self.request_id,
                        signed_evaluation_req.clone(),
                        &metadata.schema_id,
                        signer,
                    )
                    .await?
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
                                    // TODO MIRAR ESTO
                                    unreachable!();
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
                                    self.eval_req.clone()
                                {
                                    req.context.governance_id
                                } else {
                                    let e = ActorError::FunctionalFail(
                                        "Can not get eval request".to_owned(),
                                    );
                                    error!(TARGET_EVALUATION, "Response, can not get eval request: {}", e);
                                    return Err(emit_fail(ctx, e).await);
                                };

                                if let Err(e) = send_reboot_to_req(
                                    ctx,
                                    &self.request_id,
                                    governance_id,
                                )
                                .await
                                {
                                    error!(TARGET_EVALUATION, "Response, can not send reboot to Request actor: {}", e);
                                    return Err(emit_fail(ctx, e).await);
                                }
                                self.reboot = true;

                                self.end_evaluators(ctx).await;
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
                                        error!(TARGET_EVALUATION, "Response, can not create evaluation response: {}", e);
                                        return Err(emit_fail(ctx, e).await);
                                    }
                                }
                            };

                            if let Err(e) =
                                self.send_evaluation_to_req(ctx, response).await
                            {
                                error!(TARGET_EVALUATION, "Response, can send evaluation to request actor: {}", e);
                                return Err(emit_fail(ctx, e).await);
                            };
                        } else if self.evaluators.is_empty() {
                            let response = match self.fail_evaluation(ctx).await
                            {
                                Ok(res) => res,
                                Err(e) => {
                                    error!(TARGET_EVALUATION, "Response, can not create evaluation response: {}", e);
                                    return Err(emit_fail(ctx, e).await);
                                }
                            };
                            if let Err(e) =
                                self.send_evaluation_to_req(ctx, response).await
                            {
                                error!(TARGET_EVALUATION, "Response, can send evaluation to request actor: {}", e);
                                return Err(emit_fail(ctx, e).await);
                            };
                        }
                    } else {
                        warn!(TARGET_EVALUATION, "Response, A response has been received from someone we were not expecting.");
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
    use identity::identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    };
    use serde_json::json;
    use serial_test::serial;

    use crate::{
        approval::approver::ApprovalStateRes,
        model::{event::LedgerValue, HashId, Namespace, SignTypesNode},
        node::Node,
        query::{Query, QueryMessage, QueryResponse},
        request::{
            RequestHandler, RequestHandlerMessage, RequestHandlerResponse,
        },
        subject::{
            event::{LedgerEvent, LedgerEventMessage, LedgerEventResponse},
            Subject,
        },
        validation::tests::create_subject_gov,
        EventRequest, FactRequest, Governance, NodeMessage, NodeResponse,
        Signed, SubjectMessage, SubjectResponse, ValueWrapper,
    };

    #[tokio::test]
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
            payload: ValueWrapper(json!({"Patch": {
                    "data": [
            {
                "op": "add",
                "path": "/members/1",
                "value": {
                    "id": "EUrVnqpwo9EKBvMru4wWLMpJgOTKM5gZnxApRmjrRbbE",
                    "name": "KoreNode1"
                }
            }]}})),
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

        assert_eq!("In Approval", state);
        let QueryResponse::ApprovalState { request, state } = query_actor
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(state, "Pending");
        assert!(!request.is_empty());

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

        assert_eq!(res, format!("The approval request for subject {} has changed to RespondedAccepted", subject_id.to_string()));

        tokio::time::sleep(Duration::from_secs(1)).await;
        let QueryResponse::ApprovalState { state, .. } = query_actor
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(state, "RespondedAccepted");

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
            {
                "op": "add",
                "path": "/members/1",
                "value": {
                    "id": "EUrVnqpwo9EKBvMru4wWLMpJgOTKM5gZnxApRmjrRbbE",
                    "name": "KoreNode1"
                }
            }])))
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
        assert_ne!(metadata.subject_public_key, KeyIdentifier::default());
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 1);
        assert!(metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 1);
        assert!(!gov.members.is_empty());
        assert!(!gov.roles.is_empty());
        assert!(gov.schemas.is_empty());
        assert!(!gov.policies.is_empty());
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
            payload: ValueWrapper(json!({"Patch": {
                    "data": [
                        {
                            "op": "add",
                            "path": "/roles/6",
                            "value": {
                                "namespace": "",
                                "role": {
                                    "CREATOR": {
                                        "quantity": 2
                                    }
                                },
                                "schema": {
                                    "ID": "Example"
                                },
                                "who": {
                                    "NAME": "Owner"
                                }
                            }
                        },
                        {
                            "op": "add",
                            "path": "/roles/7",
                            "value": {
                                "namespace": "",
                                "role": "ISSUER",
                                "schema": {
                                    "ID": "Example"
                                },
                                "who": {
                                    "NAME": "Owner"
                                }
                            }
                        },
                        {
                            "op": "add",
                            "path": "/policies/1",
                            "value": {
                                "id": "Example",
                                "approve": {
                                    "quorum": {
                                        "FIXED": 1
                                    }
                                },
                                "evaluate": {
                                    "quorum": "MAJORITY"
                                },
                                "validate": {
                                    "quorum": "MAJORITY"
                                }
                            }
                        },
                        {
                            "op": "add",
                            "path": "/schemas/0",
                            "value": {
                                "contract": {
                                    "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS50aHJlZSA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZEFsbCB7IG9uZSwgdHdvLCB0aHJlZSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBvbmU7CiAgICAgICAgc3RhdGUudHdvID0gdHdvOwogICAgICAgIHN0YXRlLnRocmVlID0gdGhyZWU7CiAgICAgIH0KICB9CiAgY29udHJhY3RfcmVzdWx0LnN1Y2Nlc3MgPSB0cnVlOwp9Cgo="
                                },
                                "id": "Example",
                                "initial_value": {
                                    "one": 0,
                                    "two": 0,
                                    "three": 0
                                }
                            }
                        }
            ]}})),
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

        let QueryResponse::ApprovalState { request, state } = query_actor
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(state, "Pending");
        assert!(!request.is_empty());

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

        assert_eq!(res, format!("The approval request for subject {} has changed to RespondedAccepted", subject_id.to_string()));

        tokio::time::sleep(Duration::from_secs(1)).await;
        let QueryResponse::ApprovalState { state, .. } = query_actor
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(state, "RespondedAccepted");

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
            {
                "op": "add",
                "path": "/policies/1",
                "value": {
                    "id": "Example",
                    "approve": {
                        "quorum": {
                            "FIXED": 1
                        }
                    },
                    "evaluate": {
                        "quorum": "MAJORITY"
                    },
                    "validate": {
                        "quorum": "MAJORITY"
                    }
                }
            },
            {
                "op": "add",
                "path": "/roles/6",
                "value": {
                    "namespace": "",
                    "role": {
                        "CREATOR": {
                            "quantity": 2
                        }
                    },
                    "schema": {
                        "ID": "Example"
                    },
                    "who": {
                        "NAME": "Owner"
                    }
                }
            },
            {
                "op": "add",
                "path": "/roles/7",
                "value": {
                    "namespace": "",
                    "role": "ISSUER",
                    "schema": {
                        "ID": "Example"
                    },
                    "who": {
                        "NAME": "Owner"
                    }
                }
            },
            {
                "op": "add",
                "path": "/schemas/0",
                "value": {
                    "contract": {
                        "raw": "dXNlIHNlcmRlOjp7U2VyaWFsaXplLCBEZXNlcmlhbGl6ZX07Cgp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogIHB1YiBvbmU6IHUzMiwKICBwdWIgdHdvOiB1MzIsCiAgcHViIHRocmVlOiB1MzIKfQoKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSldCmVudW0gU3RhdGVFdmVudCB7CiAgTW9kT25lIHsgZGF0YTogdTMyIH0sCiAgTW9kVHdvIHsgZGF0YTogdTMyIH0sCiAgTW9kVGhyZWUgeyBkYXRhOiB1MzIgfSwKICBNb2RBbGwgeyBvbmU6IHUzMiwgdHdvOiB1MzIsIHRocmVlOiB1MzIgfQp9CgojW25vX21hbmdsZV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBldmVudF9wdHIsIGlzX293bmVyLCBjb250cmFjdF9sb2dpYykKfQoKI1tub19tYW5nbGVdCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxTdGF0ZSwgU3RhdGVFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICBsZXQgc3RhdGUgPSAmbXV0IGNvbnRyYWN0X3Jlc3VsdC5maW5hbF9zdGF0ZTsKICBtYXRjaCBjb250ZXh0LmV2ZW50IHsKICAgICAgU3RhdGVFdmVudDo6TW9kT25lIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBkYXRhOwogICAgICB9LAogICAgICBTdGF0ZUV2ZW50OjpNb2RUd28geyBkYXRhIH0gPT4gewogICAgICAgIHN0YXRlLnR3byA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZFRocmVlIHsgZGF0YSB9ID0+IHsKICAgICAgICBzdGF0ZS50aHJlZSA9IGRhdGE7CiAgICAgIH0sCiAgICAgIFN0YXRlRXZlbnQ6Ok1vZEFsbCB7IG9uZSwgdHdvLCB0aHJlZSB9ID0+IHsKICAgICAgICBzdGF0ZS5vbmUgPSBvbmU7CiAgICAgICAgc3RhdGUudHdvID0gdHdvOwogICAgICAgIHN0YXRlLnRocmVlID0gdGhyZWU7CiAgICAgIH0KICB9CiAgY29udHJhY3RfcmVzdWx0LnN1Y2Nlc3MgPSB0cnVlOwp9Cgo="
                    },
                    "id": "Example",
                    "initial_value": {
                        "one": 0,
                        "two": 0,
                        "three": 0
                    }
                }
            }])))
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
        assert_ne!(metadata.subject_public_key, KeyIdentifier::default());
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 1);
        assert!(metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 1);
        assert!(!gov.members.is_empty());
        assert!(!gov.roles.is_empty());
        assert!(!gov.schemas.is_empty());
        assert!(!gov.policies.is_empty());

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

    #[tokio::test]
    #[serial]
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

        assert_eq!("Finish", state);

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
            LedgerValue::Patch(ValueWrapper(serde_json::Value::String(
                "[]".to_owned(),
            ),))
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
        assert_eq!(metadata.genesis_gov_version, 1);
        assert_ne!(metadata.subject_public_key, KeyIdentifier::default());
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

    #[tokio::test]
    #[serial]
    async fn test_subject() {
        let _ = create_subject().await;
    }

    #[tokio::test]
    #[serial]
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
        assert_ne!(metadata.subject_public_key, KeyIdentifier::default());
        assert_eq!(metadata.schema_id, "Example");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 1);
        assert!(metadata.active);
        assert!(metadata.properties.0["one"].as_u64().unwrap() == 100);
        assert!(metadata.properties.0["two"].as_u64().unwrap() == 0);
        assert!(metadata.properties.0["three"].as_u64().unwrap() == 0);
    }
}
