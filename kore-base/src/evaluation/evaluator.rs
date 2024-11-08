// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Duration;

use crate::{
    evaluation::response::Response as EvalRes,
    governance::Schema,
    helpers::network::{intermediary::Intermediary, NetworkMessage},
    model::{
        common::{get_gov, get_sign},
        network::{RetryNetwork, TimeOutResponse},
        HashId, SignTypesNode, TimeStamp,
    },
    subject::{SubjectMessage, SubjectResponse},
    Error, EventRequest, FactRequest, Signed, Subject, ValueWrapper, CONTRACTS,
    DIGEST_DERIVATOR,
};

use crate::helpers::network::ActorMessage;

use async_trait::async_trait;
use identity::identifier::{derive::digest::DigestDerivator, KeyIdentifier};

use json_patch::diff;
use network::ComunicateInfo;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError,
    FixedIntervalStrategy, Handler, Message, RetryActor, RetryMessage,
    Strategy,
};

use serde_json::Value;
use tracing::error;

use super::{
    compiler::{Compiler, CompilerMessage, CompilerResponse},
    request::EvaluationReq,
    response::EvaluationRes,
    runner::{
        types::{Contract, GovernanceData, RunnerResult},
        Runner, RunnerMessage, RunnerResponse,
    },
    Evaluation, EvaluationMessage,
};

/// A struct representing a Evaluator actor.
#[derive(Default, Clone, Debug)]
pub struct Evaluator {
    request_id: String,
    node: KeyIdentifier,
}

impl Evaluator {
    pub fn new(request_id: String, node: KeyIdentifier) -> Self {
        Evaluator { request_id, node }
    }

    async fn execute_contract(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        state: &ValueWrapper,
        event: &ValueWrapper,
        compiled_contract: Contract,
        is_owner: bool,
    ) -> Result<RunnerResponse, ActorError> {
        let child = ctx.create_child("runner", Runner::default()).await;

        let runner_actor = match child {
            Ok(child) => child,
            Err(_e) => return Err(_e),
        };

        let response = runner_actor
            .ask(RunnerMessage {
                state: state.clone(),
                event: event.clone(),
                compiled_contract,
                is_owner,
            })
            .await;

        match response {
            Ok(eval) => Ok(eval),
            Err(_e) => Err(_e),
        }
    }

    async fn compile_contracts(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        ids: &[String],
        schemas: Vec<Schema>,
        governance_id: &str,
    ) -> Result<(), Error> {
        for id in ids {
            let schema = if let Some(schema) =
                schemas.iter().find(|x| x.id.clone() == id.clone())
            {
                schema
            } else {
                return Err(Error::Evaluation("There is a contract that requires compilation but its scheme could not be found".to_owned()));
            };

            let compiler_path = ActorPath::from(format!(
                "/user/node/{}/{}_compiler",
                governance_id, schema.id
            ));
            let compiler_actor = ctx.system().get_actor(&compiler_path).await;

            let compiler_actor: ActorRef<Compiler> =
                if let Some(compiler_actor) = compiler_actor {
                    compiler_actor
                } else {
                    return Err(Error::Actor(format!(
                        "Can not find compiler actor in {}",
                        compiler_path
                    )));
                };

            let response = compiler_actor
                .ask(CompilerMessage::CompileCheck {
                    contract: schema.contract.raw.clone(),
                    initial_value: schema.initial_value.clone(),
                    contract_path: format!("{}_{}", governance_id, id),
                })
                .await;

            let compilation = match response {
                Ok(compilation) => compilation,
                Err(e) => return Err(Error::Actor(format!("{}", e))),
            };

            match compilation {
                CompilerResponse::Check => {},
                CompilerResponse::Error(e) => return Err(e),
                _ => return Err(Error::Actor("An unexpected response has been received from compiler actor".to_owned()))
            }
        }
        Ok(())
    }

    async fn evaluate(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        execute_contract: &EvaluationReq,
        event: &FactRequest,
    ) -> Result<RunnerResult, Error> {
        // TODO: En el original se usa el sn y no el gov version, revizar.

        let is_governance = execute_contract.context.schema_id == "governance";

        // Get governance id
        let governance_id = if is_governance {
            execute_contract.context.subject_id.clone()
        } else {
            execute_contract.context.governance_id.clone()
        };

        // Get governance
        let governance = get_gov(ctx, &governance_id.to_string()).await?;
        // Get governance version
        let governance_version = governance.version;

        match governance_version.cmp(&execute_contract.gov_version) {
            std::cmp::Ordering::Equal => {
                // If it is the same it means that we have the latest version of governance, we are up to date.
            }
            std::cmp::Ordering::Greater => {
                // It is impossible to have a greater version of governance than the owner of the governance himself.
                // The only possibility is that it is an old evaluation request.
                // Hay que hacerlo TODO
            }
            std::cmp::Ordering::Less => {
                // Si es un sujeto de traabilidad hay que darle una vuelta.
                // Stop evaluation process, we need to update governance, we are out of date.
                // Hay que hacerlo TODO
            }
        }

        let contract: Contract = if is_governance {
            Contract::GovContract
        } else {
            let contracts = CONTRACTS.read().await;
            if let Some(contract) = contracts.get(&format!(
                "{}_{}",
                governance_id, execute_contract.context.schema_id
            )) {
                Contract::CompiledContract(contract.clone())
            } else {
                todo!()
            }
        };

        // Mirar la parte final de execute contract.
        let response = self
            .execute_contract(
                ctx,
                &execute_contract.context.state,
                &event.payload,
                contract,
                execute_contract.context.is_owner,
            )
            .await
            .map_err(|e| Error::Actor(format!("{}", e)))?;

        let (result, compilations) = match response {
            RunnerResponse::Response {
                result,
                compilations,
            } => (result, compilations),
            RunnerResponse::Error(e) => return Err(e),
        };

        if result.success && is_governance && !compilations.is_empty() {
            let governance_data = serde_json::from_value::<GovernanceData>(
                result.final_state.0.clone(),
            )
            .map_err(|e| {
                Error::Evaluation(format!(
                    "Can not create governance data {}",
                    e
                ))
            })?;

            self.compile_contracts(
                ctx,
                &compilations,
                governance_data.schemas,
                &governance_id.to_string(),
            )
            .await?;
        }

        Ok(result)
    }

    fn generate_json_patch(
        prev_state: &Value,
        new_state: &Value,
    ) -> Result<Value, Error> {
        let patch = diff(prev_state, new_state);
        serde_json::to_value(patch).map_err(|e| {
            Error::Evaluation(format!("Can not generate json patch {}", e))
        })
    }

    async fn build_response(
        evaluation: RunnerResult,
        evaluation_req: EvaluationReq,
    ) -> EvaluationRes {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };

        if evaluation.success {
            // Sacar esquema actual
            let governance_data =
                match serde_json::from_value::<GovernanceData>(
                    evaluation_req.context.state.0.clone(),
                )
                .map_err(|e| {
                    Error::Evaluation(format!(
                        "Can not create governance data {}",
                        e
                    ))
                }) {
                    Ok(gov_data) => gov_data,
                    Err(_e) => todo!(),
                };

            let schema = if evaluation_req.context.schema_id == "governance" {
                "governance".to_owned()
            } else {
                let schema = if let Some(schema) = governance_data
                    .schemas
                    .iter()
                    .find(|x| x.id.clone() == evaluation_req.context.schema_id)
                {
                    schema
                } else {
                    todo!()
                };

                format!(
                    "{}_{}",
                    evaluation_req.context.governance_id.clone(),
                    schema.id
                )
            };

                let state_hash = match evaluation.final_state.hash_id(derivator)
                {
                    Ok(state_hash) => state_hash,
                    Err(_e) => todo!(),
                };

                let patch = match Self::generate_json_patch(
                    &evaluation_req.context.state.0,
                    &evaluation.final_state.0,
                ) {
                    Ok(patch) => patch,
                    Err(_e) => todo!(),
                };

                return EvaluationRes::Response(EvalRes {
                    patch: ValueWrapper(patch),
                    state_hash,
                    eval_success: evaluation.success,
                    appr_required: evaluation.approval_required,
                });
        }
        // Retornar error.
        todo!()
    }
}

#[derive(Debug, Clone)]
pub enum EvaluatorMessage {
    LocalEvaluation {
        evaluation_req: EvaluationReq,
        our_key: KeyIdentifier,
    },
    NetworkEvaluation {
        request_id: String,
        evaluation_req: Signed<EvaluationReq>,
        schema: String,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        evaluation_res: Signed<EvaluationRes>,
        request_id: String,
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
                let EventRequest::Fact(state_data) =
                    &evaluation_req.event_request.content
                else {
                    // Manejar, solo se evaluan los eventos de tipo fact TODO
                    todo!()
                };

                let evaluation = match self
                    .evaluate(ctx, &evaluation_req, state_data)
                    .await
                {
                    Ok(evaluation) => {
                        Self::build_response(evaluation, evaluation_req).await
                    }
                    Err(_e) => {
                        // Falla al hacer la evaluación, lo que es el proceso, ver como se maneja TODO
                        todo!()
                    }
                };

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::EvaluationRes(evaluation.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(_e) => todo!(),
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
                    // Can not obtain parent actor
                    return Err(ActorError::Exists(evaluation_path));
                }

                ctx.stop().await;
            }
            EvaluatorMessage::NetworkEvaluation {
                request_id,
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
                        request_id,
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

                let retry = if let Ok(retry) = ctx
                    .create_child::<RetryActor<RetryNetwork>>(
                        "retry",
                        retry_actor,
                    )
                    .await
                {
                    retry
                } else {
                    todo!()
                };

                if let Err(_e) = retry.tell(RetryMessage::Retry).await {
                    todo!()
                };
            }
            EvaluatorMessage::NetworkResponse {
                evaluation_res,
                request_id,
            } => {
                if request_id == self.request_id {
                    if self.node != evaluation_res.signature.signer {
                        // Nos llegó a una validación de un nodo incorrecto!
                        todo!()
                    }

                    if let Err(_e) = evaluation_res.verify() {
                        // Hay error criptográfico en la respuesta
                        todo!()
                    }

                    // Evaluation path.
                    let evaluation_path = ctx.path().parent();

                    // Evaluation actor.
                    let evaluation_actor: Option<ActorRef<Evaluation>> =
                        ctx.system().get_actor(&evaluation_path).await;

                    if let Some(evaluation_actor) = evaluation_actor {
                        if let Err(_e) = evaluation_actor
                            .tell(EvaluationMessage::Response {
                                evaluation_res: evaluation_res.content,
                                sender: self.node.clone(),
                                signature: Some(evaluation_res.signature),
                            })
                            .await
                        {
                            // TODO error, no se puede enviar la response. Parar
                        }
                    } else {
                        // TODO no se puede obtener evaluation! Parar.
                        // Can not obtain parent actor
                    }

                    let retry = if let Some(retry) =
                        ctx.get_child::<RetryActor<RetryNetwork>>("retry").await
                    {
                        retry
                    } else {
                        todo!()
                    };
                    if let Err(_e) = retry.tell(RetryMessage::End).await {
                        todo!()
                    };
                    ctx.stop().await;
                } else {
                    // TODO llegó una respuesta con una request_id que no es la que estamos esperando, no es válido.
                }
            }
            EvaluatorMessage::NetworkRequest {
                evaluation_req,
                info,
            } => {
                if info.schema == "governance" {
                    // Aquí hay que comprobar que el owner del subject es el que envía la req.
                    let subject_path = ActorPath::from(format!(
                        "/user/node/{}",
                        evaluation_req.content.context.subject_id.clone()
                    ));
                    let subject_actor: Option<ActorRef<Subject>> =
                        ctx.system().get_actor(&subject_path).await;

                    // We obtain the evaluator
                    let response = if let Some(subject_actor) = subject_actor {
                        match subject_actor.ask(SubjectMessage::GetOwner).await
                        {
                            Ok(response) => response,
                            Err(_e) => todo!(),
                        }
                    } else {
                        todo!()
                    };

                    let subject_owner = match response {
                        SubjectResponse::Owner(owner) => owner,
                        _ => todo!(),
                    };

                    if subject_owner != evaluation_req.signature.signer {
                        // Error nos llegó una evaluation req de un nodo el cual no es el dueño
                        todo!()
                    }

                    if let Err(_e) = evaluation_req.verify() {
                        // Hay errores criptográficos
                        todo!()
                    }
                }

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("NetworkIntermediary").await;
                let mut helper = if let Some(helper) = helper {
                    helper
                } else {
                    // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                    // return Err(ActorError::Get("Error".to_owned()))
                    return Err(ActorError::NotHelper);
                };

                let EventRequest::Fact(state_data) =
                    &evaluation_req.content.event_request.content
                else {
                    // Manejar, solo se evaluan los eventos de tipo fact TODO
                    todo!()
                };

                let evaluation = match self
                    .evaluate(ctx, &evaluation_req.content, state_data)
                    .await
                {
                    Ok(evaluation) => {
                        Self::build_response(
                            evaluation,
                            evaluation_req.content.clone(),
                        )
                        .await
                    }
                    Err(_e) => {
                        // Falla al hacer la evaluación, lo que es el proceso, ver como se maneja TODO
                        todo!()
                    }
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    reciver_actor: format!(
                        "/user/node/{}/evaluation/{}",
                        evaluation_req.content.context.subject_id,
                        info.reciver.clone()
                    ),
                    schema: info.schema.clone(),
                };

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::EvaluationRes(evaluation.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(_e) => todo!(),
                };

                let signed_response: Signed<EvaluationRes> = Signed {
                    content: evaluation,
                    signature,
                };
                if let Err(_e) = helper
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
                    // error al enviar mensaje, propagar hacia arriba TODO
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
        if let ActorError::Functional(error) = error {
            if &error == "Max retries reached." {
                let evaluation_path = ctx.path().parent();

                // Evaluation actor.
                let evaluation_actor: Option<ActorRef<Evaluation>> =
                    ctx.system().get_actor(&evaluation_path).await;

                if let Some(evaluation_actor) = evaluation_actor {
                    if let Err(_e) = evaluation_actor
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
                        // TODO error, no se puede enviar la response
                        // return Err(_e);
                    }
                } else {
                    // TODO no se puede obtener evaluation! Parar.
                    // Can not obtain parent actor
                    // return Err(ActorError::Exists(evaluation_path));
                }
            }
        }
    }
}
