// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::HashSet, time::Duration};

use crate::{
    governance::{Governance, RequestStage},
    helpers::network::{intermediary::Intermediary, NetworkMessage},
    model::{signature::Signature, SignTypes, TimeStamp},
    node::{Node, NodeMessage, NodeResponse},
    subject::{SubjectCommand, SubjectResponse},
    Error, EventRequest, FactRequest, Subject, ValueWrapper,
};

use crate::helpers::network::ActorMessage;

use async_trait::async_trait;
use identity::identifier::{
    derive::digest::DigestDerivator, key_identifier, DigestIdentifier,
    KeyIdentifier,
};

use borsh::{BorshDeserialize, BorshSerialize};
use network::{Command, ComunicateInfo};
use serde::{Deserialize, Serialize};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    FixedIntervalStrategy, Handler, Message, Response, Retry, RetryStrategy,
    Strategy,
};

use tracing::{debug, error};

use super::{
    request::EvaluationReq,
    response::EvaluationRes,
    runner::{types::{Contract, ContractResult}, Runner, RunnerCommand, RunnerResponse},
    EvaluationResponse,
};

/// A struct representing a Evaluator actor.
#[derive(Default, Clone, Debug)]
pub struct Evaluator {
    request_id: String,
    finish: bool,
}

impl Evaluator {
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

        let response = runner_actor.ask(RunnerCommand {
            state: state.clone(),
            event: event.clone(),
            compiled_contract,
            is_owner,
        }).await;

        match response {
            Ok(eval) => Ok(eval),
            Err(e) => Err(e)
        }
    }

    async fn evaluate(
        &self,
        ctx: &mut ActorContext<Evaluator>,
        execute_contract: &EvaluationReq,
        event: &FactRequest,
    ) -> Result<(), Error> {
        // TODO: En el original se usa el sn y no el gov version, revizar.

        // Get governance id
        let governance_id = if &execute_contract.context.schema_id
            == "governance"
        {
            if let EventRequest::Fact(data) =
                &execute_contract.event_request.content
            {
                data.subject_id.clone()
            } else {
                return Err(Error::Validation("The only event that requires validation is the Fact event, another type of event arrived.".to_owned()));
            }
        } else {
            execute_contract.context.governance_id.clone()
        };

        // Get governance
        let governance = self.get_gov(ctx, governance_id).await?;
        // Get governance version
        let governance_version = governance.version();

        match governance_version.cmp(&execute_contract.gov_version) {
            std::cmp::Ordering::Equal => {
                // If it is the same it means that we have the latest version of governance, we are up to date.
            }
            std::cmp::Ordering::Greater => {
                // It is impossible to have a greater version of governance than the owner of the governance himself.
                // The only possibility is that it is an old validation request.
                // Hay que hacerlo TODO
            }
            std::cmp::Ordering::Less => {
                // Si es un sujeto de traabilidad hay que darle una vuelta.
                // Stop validation process, we need to update governance, we are out of date.
                // Hay que hacerlo TODO
            }
        }

        // Hash of context
        let context_hash = Self::generate_context_hash(execute_contract)?;

        // TODO: en el original sacaban el gov_version del contrato, pero tiene que ser la misma que la de la governanza, por qué se vuelve a ahacer ¿?
        let contract: Contract =
            if execute_contract.context.schema_id == "governance" {
                Contract::GovContract
            } else {
                let contract = governance
                    .get_contract(&execute_contract.context.schema_id)?;
                Contract::CompiledContract(contract)
            };

        // Sacar si es el owner
        // Mirar la parte final de execute contract.
        let response = match self.execute_contract(ctx, &execute_contract.context.state,&event.payload, contract, true).await {
            Ok(response) => response,
            Err(e) => {
                // Propagar error hacia arriba.
                todo!()
            }
        };

        

        
        // Aquí hacer la compilación si es una governanza, todo fue bien y el bool de la compilación es true.
        Ok(())
    }

    fn generate_context_hash(
        execute_contract: &EvaluationReq,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::generate_with_blake3(execute_contract)
            .map_err(|e| Error::Digest(format!("{}", e)))
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
        evaluation_req: EvaluationReq,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        evaluation_res: EvaluationRes,
        request_id: String,
    },
    NetworkRequest {
        evaluation_req: EvaluationReq,
        info: ComunicateInfo,
    },
}

impl Message for EvaluatorCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvaluatorEvent {}

impl Event for EvaluatorEvent {}

#[derive(Debug, Clone)]
pub enum EvaluatorResponse {
    Error(Error),
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
        msg: EvaluatorCommand,
        ctx: &mut ActorContext<Evaluator>,
    ) -> Result<EvaluatorResponse, ActorError> {
        Ok(EvaluatorResponse::None)
    }

    async fn on_event(
        &mut self,
        event: EvaluatorEvent,
        ctx: &mut ActorContext<Evaluator>,
    ) {
    }
}

#[async_trait]
impl Retry for Evaluator {
    type Child = RetryEvaluator;

    async fn child(
        &self,
        ctx: &mut ActorContext<Evaluator>,
    ) -> Result<ActorRef<RetryEvaluator>, ActorError> {
        ctx.create_child("network", RetryEvaluator {}).await
    }

    /// Retry message.
    async fn apply_retries(
        &self,
        ctx: &mut ActorContext<Self>,
        path: ActorPath,
        retry_strategy: &mut Strategy,
        message: <<Self as Retry>::Child as Actor>::Message,
    ) -> Result<<<Self as Retry>::Child as Actor>::Response, ActorError> {
        if let Ok(child) = self.child(ctx).await {
            let mut retries = 0;
            while retries < retry_strategy.max_retries() && !self.finish {
                debug!(
                    "Retry {}/{}.",
                    retries + 1,
                    retry_strategy.max_retries()
                );
                if let Err(e) = child.tell(message.clone()).await {
                    error!("");
                    // Manejar error del tell.
                } else {
                    if let Some(duration) = retry_strategy.next_backoff() {
                        debug!("Backoff for {:?}", &duration);
                        tokio::time::sleep(duration).await;
                    }
                    retries += 1;
                }
            }
            if self.finish {
                // LLegó respuesta se abortan los intentos.
                Ok(EvaluatorResponse::None)
            } else {
                error!("Max retries with actor {} reached.", path);
                // emitir evento de que todos los intentos fueron realizados
                Err(ActorError::ReTry)
            }
        } else {
            error!("Retries with actor {} failed. Unknown actor.", path);
            Err(ActorError::Functional(format!(
                "Retries with actor {} failed. Unknown actor.",
                path
            )))
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct RetryEvaluator {}

#[async_trait]
impl Actor for RetryEvaluator {
    type Event = EvaluatorEvent;
    type Message = NetworkMessage;
    type Response = EvaluatorResponse;
}

#[async_trait]
impl Handler<RetryEvaluator> for RetryEvaluator {
    async fn handle_message(
        &mut self,
        msg: NetworkMessage,
        ctx: &mut ActorContext<RetryEvaluator>,
    ) -> Result<EvaluatorResponse, ActorError> {
        let helper: Option<Intermediary> =
            ctx.system().get_helper("NetworkIntermediary").await;
        let mut helper = if let Some(helper) = helper {
            helper
        } else {
            // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
            // return Err(ActorError::Get("Error".to_owned()))
            return Err(ActorError::NotHelper);
        };

        if let Err(e) = helper
            .send_command(network::CommandHelper::SendMessage { message: msg })
            .await
        {
            // error al enviar mensaje, propagar hacia arriba TODO
        };
        Ok(EvaluatorResponse::None)
    }
}
