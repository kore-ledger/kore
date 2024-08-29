// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::HashSet, time::Duration};

use crate::{
    governance::{json_schema::JsonSchema, Governance, RequestStage, Schema},
    helpers::network::{intermediary::Intermediary, NetworkMessage},
    model::{network::RetryNetwork, signature::Signature, HashId, SignTypesNode, TimeStamp},
    node::{self, Node, NodeMessage, NodeResponse},
    subject::{SubjectCommand, SubjectResponse},
    Error, EventRequest, FactRequest, Signed, Subject, ValueWrapper,
    DIGEST_DERIVATOR,
};

use crate::helpers::network::ActorMessage;

use async_trait::async_trait;
use identity::identifier::{
    derive::digest::DigestDerivator, key_identifier, DigestIdentifier,
    KeyIdentifier,
};

use borsh::{BorshDeserialize, BorshSerialize};
use json_patch::diff;
use network::{Command, ComunicateInfo};
use serde::{Deserialize, Serialize};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event, FixedIntervalStrategy, Handler, Message, Response, RetryActor, RetryMessage, RetryStrategy, Strategy
};

use serde_json::{json, Value};
use tracing::{debug, error};

use super::{
    compiler::{Compiler, CompilerCommand, CompilerResponse},
    request::EvaluationReq,
    response::EvaluationRes,
    runner::{
        types::{Contract, ContractResult, GovernanceData},
        Runner, RunnerCommand, RunnerResponse,
    },
    Evaluation, EvaluationCommand, EvaluationResponse,
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

    async fn get_gov(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        governance_id: DigestIdentifier,
    ) -> Result<Governance, Error> {
        // Governance path
        let governance_path =
            ActorPath::from(format!("/user/node/{}", governance_id));

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
                "The governance actor was not found in the expected path {}",
                governance_path
            )));
        };

        match response {
            SubjectResponse::Governance(gov) => Ok(gov),
            SubjectResponse::Error(error) => {
                return Err(Error::Actor(format!("The subject encountered problems when getting governance: {}",error)));
            }
            _ => {
                return Err(Error::Actor(format!(
                    "An unexpected response has been received from node actor"
                )))
            }
        }
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
            Err(e) => return Err(e),
        };

        let response = runner_actor
            .ask(RunnerCommand {
                state: state.clone(),
                event: event.clone(),
                compiled_contract,
                is_owner,
            })
            .await;

        match response {
            Ok(eval) => Ok(eval),
            Err(e) => Err(e),
        }
    }

    async fn compile_contracts(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        ids: &[String],
        schemas: Vec<Schema>,
        governance_id: &str,
    ) -> Result<(), Error> {
        let child = ctx.create_child("compiler", Compiler::default()).await;

        let compiler_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(Error::Actor(format!("{}", e))),
        };
        for id in ids {
            let schema = if let Some(schema) =
                schemas.iter().find(|x| x.id.clone() == id.clone())
            {
                schema
            } else {
                return Err(Error::Evaluation("There is a contract that requires compilation but its scheme could not be found".to_owned()));
            };

            let response = compiler_actor
                .ask(CompilerCommand::CompileCheck {
                    contract: schema.contract.raw.clone(),
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
                _ => return Err(Error::Actor(format!("An unexpected response has been received from compiler actor")))
            }
        }
        Ok(())
    }

    async fn evaluate(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        execute_contract: &EvaluationReq,
        event: &FactRequest,
    ) -> Result<ContractResult, Error> {
        // TODO: En el original se usa el sn y no el gov version, revizar.

        let is_governance = execute_contract.context.schema_id == "governance";

        // Get governance id
        let governance_id = if is_governance {
            execute_contract.context.subject_id.clone()
        } else {
            execute_contract.context.governance_id.clone()
        };

        // Get governance
        let governance = self.get_gov(ctx, governance_id.clone()).await?;
        // Get governance version
        let governance_version = governance.get_version();

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

        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;
        let node_actor = if let Some(node_actor) = node_actor {
            node_actor
        } else {
            return Err(Error::Actor(format!(
                "The node actor was not found in the expected path {}",
                node_path
            )));
        };

        // TODO: en el original sacaban el gov_version del contrato, pero tiene que ser la misma que la de la governanza, por qué se vuelve a hacer ¿?
        let contract: Contract = if is_governance {
            Contract::GovContract
        } else {
            let response = match node_actor
                .ask(NodeMessage::CompiledContract(format!(
                    "{}_{}",
                    governance_id, execute_contract.context.schema_id
                )))
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a Subject {}",
                        e
                    )));
                }
            };

            match response {
                NodeResponse::Contract(compiled_contract) => {
                    Contract::CompiledContract(compiled_contract)
                }
                // TODO, si no encuentra el contrato se podría volver a compilar
                NodeResponse::Error(e) => {
                    return Err(Error::Actor(format!(
                        "The compiled contract could not be found: {}",
                        e
                    )))
                }
                _ => {
                    return Err(Error::Actor(format!(
                    "An unexpected response has been received from node actor"
                )))
                }
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

        if response.result.success
            && is_governance
            && !response.compilations.is_empty()
        {
            let governance_data = serde_json::from_value::<GovernanceData>(
                response.result.final_state.0.clone(),
            )
            .map_err(|e| {
                Error::Evaluation(format!(
                    "Can not create governance data {}",
                    e
                ))
            })?;

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

    fn generate_json_patch(
        prev_state: &Value,
        new_state: &Value,
    ) -> Result<Value, Error> {
        let patch = diff(prev_state, new_state);
        serde_json::to_value(patch).map_err(|e| {
            Error::Evaluation(format!("Can not generate json patch {}", e))
        })
    }

    fn build_response(
        evaluation: ContractResult,
        evaluation_req: EvaluationReq,
    ) -> Result<EvaluationRes, Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            derivator.clone()
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
                    Err(e) => todo!(),
                };

            let schema = if let Some(schema) = governance_data
                .schemas
                .iter()
                .find(|x| x.id.clone() == evaluation_req.context.schema_id)
            {
                schema
            } else {
                todo!()
            };

            // TODO las compilaciones se deben guardar en memoria, en la governanza, en un Option
            let json_schema = match JsonSchema::compile(&schema.schema) {
                Ok(json_schema) => json_schema,
                Err(e) => todo!(),
            };

            let value =
                match serde_json::to_value(evaluation.final_state.clone()) {
                    Ok(value) => value,
                    Err(e) => todo!(),
                };
            if json_schema.validate(&value) {
                let state_hash = match evaluation.final_state.hash_id(derivator)
                {
                    Ok(state_hash) => state_hash,
                    Err(e) => todo!(),
                };

                let patch = match Self::generate_json_patch(
                    &evaluation_req.context.state.0,
                    &evaluation.final_state.0,
                ) {
                    Ok(patch) => patch,
                    Err(e) => todo!(),
                };

                return Ok(EvaluationRes {
                    patch: ValueWrapper(patch),
                    state_hash,
                    eval_success: evaluation.success,
                    appr_required: evaluation.approval_required,
                });
            }
        }
        let state_hash = match evaluation_req.context.state.hash_id(derivator) {
            Ok(state_hash) => state_hash,
            Err(e) => todo!(),
        };

        return Ok(EvaluationRes {
            patch: ValueWrapper(serde_json::Value::String("[]".to_owned())),
            state_hash,
            eval_success: false,
            appr_required: false,
        });
    }
}

#[derive(Debug, Clone)]
pub enum EvaluatorCommand {
    LocalEvaluation {
        evaluation_req: EvaluationReq,
        our_key: KeyIdentifier,
    },
    NetworkEvaluation {
        request_id: String,
        evaluation_req: Signed<EvaluationReq>,
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

impl Message for EvaluatorCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvaluatorEvent {}

impl Event for EvaluatorEvent {}

#[derive(Debug, Clone)]
pub enum EvaluatorResponse {
    None,
}

impl Response for EvaluatorResponse {}

#[async_trait]
impl Actor for Evaluator {
    type Event = EvaluatorEvent;
    type Message = EvaluatorCommand;
    type Response = EvaluatorResponse;
}

#[async_trait]
impl Handler<Evaluator> for Evaluator {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: EvaluatorCommand,
        ctx: &mut ActorContext<Evaluator>,
    ) -> Result<EvaluatorResponse, ActorError> {
        match msg {
            EvaluatorCommand::LocalEvaluation {
                evaluation_req,
                our_key,
            } => {
                let EventRequest::Fact(state_data) =
                    &evaluation_req.event_request.content
                else {
                    // Manejar, solo se evaluan los eventos de tipo fact TODO
                    todo!()
                };

                let evaluation =
                    match self.evaluate(ctx, &evaluation_req, state_data).await
                    {
                        Ok(evaluation) => evaluation,
                        Err(e) => {
                            // Falla al hacer la evaluación, lo que es el proceso, ver como se maneja TODO
                            todo!()
                        }
                    };

                let evaluation =
                    match Self::build_response(evaluation, evaluation_req) {
                        Ok(evaluation) => evaluation,
                        Err(e) => {
                            todo!()
                        }
                    };

                // Evaluatiob path.
                let evaluation_path = ctx.path().parent();

                let evaluation_actor: Option<ActorRef<Evaluation>> =
                    ctx.system().get_actor(&evaluation_path).await;

                // Send response of evaluation to parent
                if let Some(evaluation_actor) = evaluation_actor {
                    if let Err(e) = evaluation_actor
                        .tell(EvaluationCommand::Response(evaluation))
                        .await
                    {
                        return Err(e);
                    }
                } else {
                    // Can not obtain parent actor
                    return Err(ActorError::Exists(evaluation_path));
                }

                ctx.stop().await;
            }
            EvaluatorCommand::NetworkEvaluation {
                request_id,
                evaluation_req,
                node_key,
                our_key,
            } => {
                // Lanzar evento donde lanzar los retrys
                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id,
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor: format!(
                            "/user/node/{}/evaluator",
                            evaluation_req.content.context.subject_id
                        ),
                    },
                    message: ActorMessage::EvaluationReq(evaluation_req),
                };

                let target = RetryNetwork::default();

                // TODO, la evaluación, si hay compilación podría tardar más
                let strategy = Strategy::FixedInterval(FixedIntervalStrategy::new(
                    3,
                    Duration::from_secs(5),
                ));

                let retry_actor = RetryActor::new(
                    target,
                    message,
                    strategy,
                );

                let retry = if let Ok(retry) = ctx
                .create_child::<RetryActor<RetryNetwork>>("retry", retry_actor)
                .await {
                    retry
                } else {

                    todo!()
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    todo!()
                };
            }
            EvaluatorCommand::NetworkResponse {
                evaluation_res,
                request_id,
            } => {
                if request_id == self.request_id {
                    if self.node != evaluation_res.signature.signer {
                        // Nos llegó a una validación de un nodo incorrecto!
                        todo!()
                    }

                    if let Err(e) = evaluation_res.verify() {
                        // Hay error criptográfico en la respuesta
                        todo!()
                    }

                    // Evaluation path.
                    let evaluation_path = ctx.path().parent();

                    // Evaluation actor.
                    let evaluation_actor: Option<ActorRef<Evaluation>> =
                        ctx.system().get_actor(&evaluation_path).await;

                    if let Some(evaluation_actor) = evaluation_actor {
                        if let Err(e) = evaluation_actor
                            .tell(EvaluationCommand::Response(evaluation_res.content))
                            .await
                        {
                            // TODO error, no se puede enviar la response. Parar
                        }
                    } else {
                        // TODO no se puede obtener evaluation! Parar.
                        // Can not obtain parent actor
                    }

                    let retry = if let Some(retry) = ctx.get_child::<RetryActor<RetryNetwork>>("retry").await {
                        retry
                    } else {
                        todo!()
                    };
                    if let Err(e) = retry.tell(RetryMessage::End).await {
                        todo!()
                    };
                    ctx.stop().await;
                } else {
                    // TODO llegó una respuesta con una request_id que no es la que estamos esperando, no es válido.
                }
            },
            EvaluatorCommand::NetworkRequest {
                evaluation_req,
                info,
            } => {
                // Aquí hay que comprobar que el owner del subject es el que envía la req.
                let subject_path = ActorPath::from(format!(
                    "/user/node/{}",
                    evaluation_req.content.context.subject_id.clone()
                ));
                let subject_actor: Option<ActorRef<Subject>> =
                    ctx.system().get_actor(&subject_path).await;

                // We obtain the validator
                let response = if let Some(subject_actor) = subject_actor {
                    match subject_actor.ask(SubjectCommand::GetOwner).await {
                        Ok(response) => response,
                        Err(e) => todo!(),
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

                if let Err(e) = evaluation_req.verify() {
                    // Hay errores criptográficos
                    todo!()
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
                    Ok(evaluation) => evaluation,
                    Err(e) => {
                        // Falla al hacer la evaluación, lo que es el proceso, ver como se maneja TODO
                        todo!()
                    }
                };

                let evaluation = match Self::build_response(
                    evaluation,
                    evaluation_req.content.clone(),
                ) {
                    Ok(evaluation) => evaluation,
                    Err(e) => {
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
                };

                let node_path = ActorPath::from("/user/node");
                let node_actor: Option<ActorRef<Node>> =
                    ctx.system().get_actor(&node_path).await;

                let node_response = if let Some(node_actor) = node_actor {
                    match node_actor
                        .ask(NodeMessage::SignRequest(
                            SignTypesNode::EvaluationRes(evaluation.clone()),
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

                let signed_response: Signed<EvaluationRes> = Signed {
                    content: evaluation,
                    signature,
                };
                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::EvaluationRes(
                                signed_response,
                            ),
                        },
                    })
                    .await
                {
                    // error al enviar mensaje, propagar hacia arriba TODO
                };
            }
        }

        Ok(EvaluatorResponse::None)
    }

    async fn on_event(
        &mut self,
        event: EvaluatorEvent,
        ctx: &mut ActorContext<Evaluator>,
    ) {
    }
}
