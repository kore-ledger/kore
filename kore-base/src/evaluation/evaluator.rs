// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Duration;

use crate::{
    CONTRACTS, DIGEST_DERIVATOR, Error, EventRequest, Signed, Subject,
    ValueWrapper,
    config::Config,
    evaluation::response::Response as EvalRes,
    governance::{Governance, Schema},
    helpers::network::{NetworkMessage, intermediary::Intermediary},
    model::{
        HashId, Namespace, SignTypesNode, TimeStamp,
        common::{
            UpdateData, emit_fail, get_gov, get_metadata, get_sign,
            update_ledger_network,
        },
        network::{RetryNetwork, TimeOutResponse},
    },
    subject::{SubjectMessage, SubjectResponse},
};

use crate::helpers::network::ActorMessage;

use async_trait::async_trait;
use identity::identifier::{
    DigestIdentifier, KeyIdentifier, derive::digest::DigestDerivator,
};

use json_patch::diff;
use network::ComunicateInfo;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    FixedIntervalStrategy, Handler, Message, RetryActor, RetryMessage,
    Strategy,
};

use serde_json::Value;
use tracing::{error, warn};

use super::{
    Evaluation, EvaluationMessage,
    compiler::{Compiler, CompilerMessage},
    request::EvaluationReq,
    response::EvaluationRes,
    runner::{
        Runner, RunnerMessage, RunnerResponse,
        types::{EvaluateType, RunnerResult},
    },
};

const TARGET_EVALUATOR: &str = "Kore-Evaluation-Evaluator";

/// A struct representing a Evaluator actor.
#[derive(Default, Clone, Debug)]
pub struct Evaluator {
    request_id: String,
    version: u64,
    node: KeyIdentifier,
}

impl Evaluator {
    pub fn new(request_id: String, version: u64, node: KeyIdentifier) -> Self {
        Evaluator {
            request_id,
            version,
            node,
        }
    }

    async fn execute_contract(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        state: &ValueWrapper,
        evaluate_type: EvaluateType,
        is_owner: bool,
    ) -> Result<RunnerResponse, ActorError> {
        let runner_actor =
            ctx.create_child("runner", Runner::default()).await?;

        runner_actor
            .ask(RunnerMessage {
                state: state.clone(),
                evaluate_type,
                is_owner,
            })
            .await
    }

    async fn compile_contracts(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        ids: &[String],
        schemas: Vec<Schema>,
        governance_id: &str,
    ) -> Result<(), ActorError> {
        let Some(config): Option<Config> =
            ctx.system().get_helper("config").await
        else {
            return Err(ActorError::NotHelper("config".to_owned()));
        };

        for id in ids {
            let schema = if let Some(schema) =
                schemas.iter().find(|x| x.id.clone() == id.clone())
            {
                schema
            } else {
                return Err(ActorError::Functional("There is a contract that requires compilation but its scheme could not be found".to_owned()));
            };

            let compiler_path = ActorPath::from(format!(
                "/user/node/{}/{}_compiler",
                governance_id, schema.id
            ));
            let compiler_actor: Option<ActorRef<Compiler>> =
                ctx.system().get_actor(&compiler_path).await;

            if let Some(compiler_actor) = compiler_actor {
                compiler_actor
                    .ask(CompilerMessage::Compile {
                        contract_name: format!("{}_{}", governance_id, id),
                        contract: schema.contract.raw.clone(),
                        initial_value: schema.initial_value.clone(),
                        contract_path: format!(
                            "{}/contracts/{}_{}",
                            config.contracts_dir, governance_id, id
                        ),
                    })
                    .await?
            } else {
                return Err(ActorError::NotFound(compiler_path));
            };
        }
        Ok(())
    }

    async fn check_governance(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        governance_id: DigestIdentifier,
        gov_version: u64,
        our_node: KeyIdentifier,
    ) -> Result<bool, ActorError> {
        let governance_string = governance_id.to_string();
        let governance = get_gov(ctx, &governance_string).await?;
        let metadata = get_metadata(ctx, &governance_string).await?;

        match gov_version.cmp(&governance.version) {
            std::cmp::Ordering::Equal => {
                // If it is the same it means that we have the latest version of governance, we are up to date.
            }
            std::cmp::Ordering::Greater => {
                // Me llega una versión mayor a la mía.
                let data = UpdateData {
                    sn: metadata.sn,
                    gov_version: governance.version,
                    subject_id: governance_id,
                    our_node,
                    other_node: self.node.clone(),
                };
                update_ledger_network(ctx, data).await?;
                let e = ActorError::Functional(
                    "Abort evaluation, update is required".to_owned(),
                );
                return Err(e);
            }
            std::cmp::Ordering::Less => {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn evaluate(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        evaluation_req: &EvaluationReq,
        governance_id: DigestIdentifier,
        is_governance: bool,
    ) -> Result<RunnerResult, ActorError> {
        let (evaluate_type, state) = match evaluation_req
            .event_request
            .content
            .clone()
        {
            EventRequest::Fact(fact_event) => {
                if is_governance {
                    (
                        EvaluateType::GovFact {
                            payload: fact_event.payload,
                        },
                        evaluation_req.state.clone(),
                    )
                } else {
                    let contracts = { CONTRACTS.read().await };

                    if let Some(contract) = contracts.get(&format!(
                        "{}_{}",
                        governance_id, evaluation_req.context.schema_id
                    )) {
                        (
                            EvaluateType::NotGovFact {
                                contract: contract.to_vec(),
                                payload: fact_event.payload,
                            },
                            evaluation_req.state.clone(),
                        )
                    } else {
                        return Err(ActorError::Functional(
                            "Contract not found".to_owned(),
                        ));
                    }
                }
            }
            EventRequest::Transfer(transfer_event) => {
                if !is_governance {
                    (
                        EvaluateType::NotGovTransfer {
                            new_owner: transfer_event.new_owner,
                            namespace: Namespace::from(
                                evaluation_req.context.namespace.clone(),
                            ),
                            schema_id: evaluation_req.context.schema_id.clone(),
                        },
                        evaluation_req.gov.clone(),
                    )
                } else {
                    (
                        EvaluateType::GovTransfer {
                            new_owner: transfer_event.new_owner,
                        },
                        evaluation_req.state.clone(),
                    )
                }
            }
            EventRequest::Confirm(confirm_request) => {
                if !is_governance {
                    let e = "Confirm event in trazability subjects do not need be evaluate";
                    return Err(ActorError::Functional(e.to_owned()));
                }
                if let Some(new_owner) = evaluation_req.new_owner.clone() {
                    (
                        EvaluateType::GovConfirm {
                            old_owner_name: confirm_request.name_old_owner,
                            new_owner,
                        },
                        evaluation_req.state.clone(),
                    )
                } else {
                    let e = "New Owner is empty in Confirm event";
                    return Err(ActorError::Functional(e.to_owned()));
                }
            }
            _ => {
                let e = "The only event that can be evaluated is the Fact, Transfer and Confirm event";
                error!(TARGET_EVALUATOR, "LocalEvaluation, {}", e);
                return Err(ActorError::Functional(e.to_owned()));
            }
        };

        // Mirar la parte final de execute contract.
        let response = self
            .execute_contract(
                ctx,
                &state,
                evaluate_type,
                evaluation_req.context.is_owner,
            )
            .await?;

        if is_governance && !response.compilations.is_empty() {
            let governance_data = serde_json::from_value::<Governance>(
                response.result.final_state.0.clone(),
            )
            .map_err(|e| {
                ActorError::Functional(format!(
                    "Can not create governance data {}",
                    e
                ))
            })?;

            // TODO SI falla eliminar los new_compilers y borrar de CONTRACTS.
            let _new_compilers = match Evaluator::create_compilers(
                ctx,
                &response.compilations,
                &governance_id.to_string(),
            )
            .await
            {
                Ok(new_compilers) => new_compilers,
                Err(e) => return Err(ActorError::Functional(format!("{}", e))),
            };

            self.compile_contracts(
                ctx,
                &response.compilations,
                governance_data.schemas,
                &governance_id.to_string(),
            )
            .await?;
        }

        Ok(response.result)
    }

    async fn create_compilers(
        ctx: &mut ActorContext<Evaluator>,
        ids: &[String],
        gov: &str,
    ) -> Result<Vec<String>, ActorError> {
        let subject_path = ActorPath::from(format!("/user/node/{}", gov));
        let subject: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject) = subject {
            subject
                .ask(SubjectMessage::CreateCompilers(ids.to_vec()))
                .await?
        } else {
            return Err(ActorError::NotFound(subject_path));
        };

        match response {
            SubjectResponse::NewCompilers(new_compilers) => Ok(new_compilers),
            _ => Err(ActorError::UnexpectedResponse(
                subject_path,
                "SubjectResponse::NewCompilers".to_owned(),
            )),
        }
    }

    fn generate_json_patch(
        prev_state: &Value,
        new_state: &Value,
    ) -> Result<Value, Error> {
        let patch = diff(prev_state, new_state);
        serde_json::to_value(patch).map_err(|e| {
            Error::JSONPatch(format!("Can not generate json patch {}", e))
        })
    }

    async fn build_response(
        evaluation: RunnerResult,
        evaluation_req: EvaluationReq,
        is_governance: bool,
    ) -> EvaluationRes {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!(TARGET_EVALUATOR, "Error getting derivator");
            DigestDerivator::Blake3_256
        };

        let (patch, state_hash) = match evaluation_req
            .event_request
            .content
            .clone()
        {
            EventRequest::Fact(..) => {
                let state_hash = match evaluation.final_state.hash_id(derivator)
                {
                    Ok(state_hash) => state_hash,
                    Err(e) => return EvaluationRes::Error(e.to_string()),
                };

                let patch = match Self::generate_json_patch(
                    &evaluation_req.state.0,
                    &evaluation.final_state.0,
                ) {
                    Ok(patch) => patch,
                    Err(e) => return EvaluationRes::Error(e.to_string()),
                };

                (ValueWrapper(patch), state_hash)
            }
            EventRequest::Transfer(..) => {
                let state_hash = match evaluation_req.state.hash_id(derivator) {
                    Ok(state_hash) => state_hash,
                    Err(e) => return EvaluationRes::Error(e.to_string()),
                };

                (evaluation.final_state, state_hash)
            }
            EventRequest::Confirm(..) => {
                if !is_governance {
                    let state_hash = match evaluation_req
                        .state
                        .hash_id(derivator)
                    {
                        Ok(state_hash) => state_hash,
                        Err(e) => return EvaluationRes::Error(e.to_string()),
                    };

                    (evaluation.final_state, state_hash)
                } else {
                    let state_hash = match evaluation
                        .final_state
                        .hash_id(derivator)
                    {
                        Ok(state_hash) => state_hash,
                        Err(e) => return EvaluationRes::Error(e.to_string()),
                    };

                    let patch = match Self::generate_json_patch(
                        &evaluation_req.state.0,
                        &evaluation.final_state.0,
                    ) {
                        Ok(patch) => patch,
                        Err(e) => return EvaluationRes::Error(e.to_string()),
                    };

                    (ValueWrapper(patch), state_hash)
                }
            }
            _ => {
                unreachable!()
            }
        };

        EvaluationRes::Response(EvalRes {
            patch,
            state_hash,
            appr_required: evaluation.approval_required,
        })
    }
}

#[derive(Debug, Clone)]
pub enum EvaluatorMessage {
    LocalEvaluation {
        evaluation_req: EvaluationReq,
        our_key: KeyIdentifier,
    },
    NetworkEvaluation {
        evaluation_req: Signed<EvaluationReq>,
        schema: String,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        evaluation_res: Signed<EvaluationRes>,
        request_id: String,
        version: u64,
    },
    NetworkRequest {
        evaluation_req: Signed<EvaluationReq>,
        info: ComunicateInfo,
    },
}

impl Message for EvaluatorMessage {}

#[async_trait]
impl Actor for Evaluator {
    type Event = ();
    type Message = EvaluatorMessage;
    type Response = ();
}

#[async_trait]
impl Handler<Evaluator> for Evaluator {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: EvaluatorMessage,
        ctx: &mut ActorContext<Evaluator>,
    ) -> Result<(), ActorError> {
        match msg {
            EvaluatorMessage::LocalEvaluation {
                evaluation_req,
                our_key,
            } => {
                let is_governance =
                    evaluation_req.context.schema_id == "governance";

                // Get governance id
                let governance_id = if is_governance {
                    evaluation_req.context.subject_id.clone()
                } else {
                    evaluation_req.context.governance_id.clone()
                };

                let evaluation = match self
                    .evaluate(
                        ctx,
                        &evaluation_req,
                        governance_id,
                        is_governance,
                    )
                    .await
                {
                    Ok(evaluation) => {
                        Self::build_response(
                            evaluation,
                            evaluation_req,
                            is_governance,
                        )
                        .await
                    }
                    Err(e) => {
                        error!(
                            TARGET_EVALUATOR,
                            "LocalEvaluation, can not build evaluator response: {}",
                            e
                        );
                        if let ActorError::Functional(_) = e {
                            EvaluationRes::Error(e.to_string())
                        } else {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                };

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::EvaluationRes(evaluation.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        error!(
                            TARGET_EVALUATOR,
                            "LocalEvaluation, can not sign evaluator response: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                // Evaluatiob path.
                let evaluation_path = ctx.path().parent();

                let evaluation_actor: Option<ActorRef<Evaluation>> =
                    ctx.system().get_actor(&evaluation_path).await;

                // Send response of evaluation to parent
                if let Some(evaluation_actor) = evaluation_actor {
                    evaluation_actor
                        .tell(EvaluationMessage::Response {
                            evaluation_res: evaluation,
                            sender: our_key,
                            signature: Some(signature),
                        })
                        .await?
                } else {
                    error!(
                        TARGET_EVALUATOR,
                        "LocalEvaluation, can not obtain Evaluation actor"
                    );
                    return Err(ActorError::Exists(evaluation_path));
                }

                ctx.stop().await;
            }
            EvaluatorMessage::NetworkEvaluation {
                evaluation_req,
                schema,
                node_key,
                our_key,
            } => {
                let reciver_actor = if schema == "governance" {
                    format!(
                        "/user/node/{}/evaluator",
                        evaluation_req.content.context.subject_id
                    )
                } else {
                    format!(
                        "/user/node/{}/{}_evaluation",
                        evaluation_req.content.context.governance_id, schema
                    )
                };

                // Lanzar evento donde lanzar los retrys
                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id: self.request_id.clone(),
                        version: self.version,
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor,
                        schema,
                    },
                    message: ActorMessage::EvaluationReq {
                        req: evaluation_req,
                    },
                };

                let target = RetryNetwork::default();

                // TODO, la evaluación, si hay compilación podría tardar más
                let strategy = Strategy::FixedInterval(
                    FixedIntervalStrategy::new(3, Duration::from_secs(5)),
                );

                let retry_actor = RetryActor::new(target, message, strategy);

                let retry = match ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                {
                    Ok(retry) => retry,
                    Err(e) => {
                        error!(
                            TARGET_EVALUATOR,
                            "NetworkEvaluation, can not create Retry actor"
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    error!(
                        TARGET_EVALUATOR,
                        "NetworkEvaluation, can not send retry message to Retry actor"
                    );
                    return Err(emit_fail(ctx, e).await);
                };
            }
            EvaluatorMessage::NetworkResponse {
                evaluation_res,
                request_id,
                version,
            } => {
                if request_id == self.request_id && version == self.version {
                    if self.node != evaluation_res.signature.signer {
                        let e = "We received an evaluation where the request indicates one subject but the info indicates another.";
                        error!(TARGET_EVALUATOR, "NetworkResponse, {}", e);
                        return Err(ActorError::Functional(e.to_owned()));
                    }

                    if let Err(e) = evaluation_res.verify() {
                        error!(
                            TARGET_EVALUATOR,
                            "NetworkResponse, Can not verify signature: {}", e
                        );
                        return Err(ActorError::Functional(format!(
                            "Can not verify signature: {}",
                            e
                        )));
                    }

                    // Evaluation path.
                    let evaluation_path = ctx.path().parent();

                    // Evaluation actor.
                    let evaluation_actor: Option<ActorRef<Evaluation>> =
                        ctx.system().get_actor(&evaluation_path).await;

                    if let Some(evaluation_actor) = evaluation_actor {
                        if let Err(e) = evaluation_actor
                            .tell(EvaluationMessage::Response {
                                evaluation_res: evaluation_res.content,
                                sender: self.node.clone(),
                                signature: Some(evaluation_res.signature),
                            })
                            .await
                        {
                            error!(
                                TARGET_EVALUATOR,
                                "NetworkResponse, Can not send response to Evaluation actor: {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    } else {
                        error!(
                            TARGET_EVALUATOR,
                            "NetworkResponse, Can not obtain Evaluation actor"
                        );
                        let e = ActorError::NotFound(evaluation_path);
                        return Err(emit_fail(ctx, e).await);
                    }

                    'retry: {
                        let Some(retry) = ctx
                            .get_child::<RetryActor<RetryNetwork>>("retry")
                            .await
                        else {
                            // Aquí me da igual, porque al parar este actor para el hijo
                            break 'retry;
                        };

                        if let Err(e) = retry.tell(RetryMessage::End).await {
                            error!(
                                TARGET_EVALUATOR,
                                "Can not end Retry actor: {}", e
                            );
                            // Aquí me da igual, porque al parar este actor para el hijo
                            break 'retry;
                        };
                    }

                    ctx.stop().await;
                } else {
                    warn!(
                        TARGET_EVALUATOR,
                        "NetworkResponse, invalid request id"
                    );
                }
            }
            EvaluatorMessage::NetworkRequest {
                evaluation_req,
                info,
            } => {
                let info_subject_path =
                    ActorPath::from(info.reciver_actor.clone()).parent().key();
                // Nos llegó una eval donde en la request se indica un sujeto pero en el info otro
                // Posible ataque.
                let governance_id = if evaluation_req
                    .content
                    .context
                    .governance_id
                    .is_empty()
                {
                    evaluation_req.content.context.subject_id.clone()
                } else {
                    evaluation_req.content.context.governance_id.clone()
                };

                if info_subject_path != governance_id.to_string() {
                    let e = "We received an evaluation where the request indicates one subject but the info indicates another.";
                    error!(TARGET_EVALUATOR, "NetworkRequest, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                if let Err(e) = evaluation_req.verify() {
                    let e =
                        format!("Can not verify signature of request: {}", e);
                    error!(TARGET_EVALUATOR, "NetworkRequest, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    let e = ActorError::NotHelper("network".to_owned());
                    error!(
                        TARGET_EVALUATOR,
                        "NetworkRequest, Can not get network helper: {}", e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                let is_governance =
                    evaluation_req.content.context.schema_id == "governance";

                let reboot = match self
                    .check_governance(
                        ctx,
                        governance_id.clone(),
                        evaluation_req.content.gov_version,
                        info.reciver.clone(),
                    )
                    .await
                {
                    Ok(reboot) => reboot,
                    Err(e) => {
                        error!(
                            TARGET_EVALUATOR,
                            "NetworkRequest, Can not check governance: {}", e
                        );
                        if let ActorError::Functional(_) = e {
                            return Err(e);
                        } else {
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                };

                let evaluation = if reboot {
                    EvaluationRes::Reboot
                } else {
                    match self
                        .evaluate(
                            ctx,
                            &evaluation_req.content,
                            governance_id,
                            is_governance,
                        )
                        .await
                    {
                        Ok(evaluation) => {
                            Self::build_response(
                                evaluation,
                                evaluation_req.content.clone(),
                                is_governance,
                            )
                            .await
                        }
                        Err(e) => {
                            error!(
                                TARGET_EVALUATOR,
                                "NetworkRequest, Can not build response: {}", e
                            );
                            if let ActorError::Functional(_) = e {
                                EvaluationRes::Error(e.to_string())
                            } else {
                                return Err(emit_fail(ctx, e).await);
                            }
                        }
                    }
                };

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::EvaluationRes(evaluation.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        error!(
                            TARGET_EVALUATOR,
                            "NetworkRequest, Can not sign response: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    version: info.version,
                    reciver_actor: format!(
                        "/user/node/{}/evaluation/{}",
                        evaluation_req.content.context.subject_id,
                        info.reciver.clone()
                    ),
                    schema: info.schema.clone(),
                };

                let signed_response: Signed<EvaluationRes> = Signed {
                    content: evaluation,
                    signature,
                };
                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::EvaluationRes {
                                res: signed_response,
                            },
                        },
                    })
                    .await
                {
                    error!(
                        TARGET_EVALUATOR,
                        "NetworkRequest, can not send response to network: {}",
                        e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if info.schema != "governance" {
                    ctx.stop().await;
                }
            }
        }

        Ok(())
    }

    async fn on_child_error(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Evaluator>,
    ) {
        match error {
            ActorError::ReTry => {
                let evaluation_path = ctx.path().parent();

                // Evaluation actor.
                let evaluation_actor: Option<ActorRef<Evaluation>> =
                    ctx.system().get_actor(&evaluation_path).await;

                if let Some(evaluation_actor) = evaluation_actor {
                    if let Err(e) = evaluation_actor
                        .tell(EvaluationMessage::Response {
                            evaluation_res: EvaluationRes::TimeOut(
                                TimeOutResponse {
                                    re_trys: 3,
                                    timestamp: TimeStamp::now(),
                                    who: self.node.clone(),
                                },
                            ),
                            signature: None,
                            sender: self.node.clone(),
                        })
                        .await
                    {
                        error!(
                            TARGET_EVALUATOR,
                            "OnChildError, can not send response to Evaluation actor: {}",
                            e
                        );
                        emit_fail(ctx, e).await;
                    }
                } else {
                    let e = ActorError::NotFound(evaluation_path);
                    error!(
                        TARGET_EVALUATOR,
                        "OnChildError, can not obtain Evaluation actor: {}", e
                    );
                    emit_fail(ctx, e).await;
                }
                ctx.stop().await;
            }
            _ => {
                error!(TARGET_EVALUATOR, "OnChildError, unexpected error");
            }
        };
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Evaluator>,
    ) -> ChildAction {
        error!(TARGET_EVALUATOR, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}
