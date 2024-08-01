// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::HashSet, fmt::format};

use crate::{
    governance::{
        Governance, GovernanceCommand, GovernanceResponse, RequestStage,
    },
    model::{signature::Signature, SignTypes},
    node::{Node, NodeMessage, NodeResponse},
    Error,
};

use super::{
    proof::ValidationProof, request::{SignersRes, ValidationReq}, response::ValidationRes, Validation, ValidationCommand, ValidationResponse
};

use async_trait::async_trait;
use identity::identifier::{
    derive::digest::DigestDerivator, key_identifier, DigestIdentifier,
    KeyIdentifier,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event,
    Handler, Message, Response, Retry, RetryStrategy, Strategy,
};

use tracing::{debug, error};

/// A struct representing a validator actor.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    finish: bool,
}

impl Validator {
    async fn validation_event(
        &self,
        ctx: &mut ActorContext<Validator>,
        validation_req: ValidationReq,
    ) -> Result<Signature, Error> {
        // Obtain gov_version
        // If is a create of governance (governance does not exist until validation is complete)
        let actual_gov_version: u64 = if validation_req.proof.schema_id
            == "governance"
            && validation_req.proof.sn == 0
        {
            0
        // ask the government for its version
        } else {
            // Governance path
            let governance_path = ActorPath::from(format!(
                "/user/node/{}",
                validation_req.proof.governance_id
            ));
            // Governance actor.
            let governance_actor: Option<ActorRef<Governance>> =
                ctx.system().get_actor(&governance_path).await;

            // We obtain the actor governance
            let response = if let Some(governance_actor) = governance_actor {
                // We ask a node
                let response =
                    governance_actor.ask(GovernanceCommand::GetVersion).await;
                match response {
                    Ok(response) => response,
                    Err(e) => {
                        return Err(Error::Actor(format!(
                            "Error when asking a governance {}",
                            e
                        )));
                    }
                }
            } else {
                return Err(Error::Actor(format!("The governance actor was not found in the expected path /user/node/{}",  validation_req.proof.governance_id)));
            };

            match response {
            GovernanceResponse::Version(version) => version,
            _ => return Err(Error::Actor(format!("An unexpected response has been received from governance actor")))
            }
        };
        match actual_gov_version.cmp(&validation_req.proof.governance_version) {
            std::cmp::Ordering::Equal => {
                // If it is the same it means that we have the latest version of governance, we are up to date.
            }
            std::cmp::Ordering::Greater => {
                // It is impossible to have a greater version of governance than the owner of the governance himself.
                // The only possibility is that it is an old validation request.
                // Hay que hacerlo
            }
            std::cmp::Ordering::Less => {
                // Stop validation process, we need to update governance, we are out of date.
                // Hay que hacerlo
            }
        }

        // Verify subject's signature on proof
        if let Err(error) = validation_req
            .subject_signature
            .verify(&validation_req.proof)
        {
            return Err(error);
        }

        let subject_public_key = self
            .check_proofs(
                ctx,
                &validation_req.proof,
                validation_req.previous_proof,
                validation_req.prev_event_validation_response,
            )
            .await?;
        // TODO: verify this, if you rotate the cryptographic material they will not match?
        if validation_req.subject_signature.signer != subject_public_key {
            error!("");
            return Err(Error::Validation(format!("KeyIdentifier of the subject signature does not match the KeyIdentifier of the check_proof")));
        }

        // Node path.
        let node_path = ActorPath::from("/user/node");
        // Node actor.
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        // We obtain the actor node
        let response = if let Some(node_actor) = node_actor {
            // We ask a node
            let response = node_actor
                .ask(NodeMessage::SignRequest(SignTypes::Validation(
                    validation_req.proof,
                )))
                .await;
            match response {
                Ok(response) => response,
                Err(e) => {
                    return Err(Error::Actor(format!(
                        "Error when asking a node {}",
                        e
                    )));
                }
            }
        } else {
            return Err(Error::Actor(format!(
                "The node actor was not found in the expected path /user/node"
            )));
        };

        // We handle the possible responses of node
        match response {
            NodeResponse::SignRequest(sign) => {
                Ok(sign)
            }
            NodeResponse::Error(error) => {
                Err(Error::Actor(format!(
                    "The node encountered problems when signing the proof: {}",
                    error
                )))
            }
            _ => Err(Error::Actor(format!(
                "An unexpected response has been received from node actor"
            ))),
        }
    }

    async fn check_proofs(
        &self,
        ctx: &mut ActorContext<Validator>,
        new_proof: &ValidationProof,
        previous_proof: Option<ValidationProof>,
        previous_validation_signatures: Vec<SignersRes>,
    ) -> Result<KeyIdentifier, Error> {
        // Not genesis event
        if let Some(previous_proof) = previous_proof {
            // subject_public_key is not verified because it can change if a transfer of the subject is made. is correct?
            // Governance_version can be the same or not, if in the last event gov was changed
            if previous_proof.event_hash != new_proof.prev_event_hash
                || previous_proof.sn + 1 != new_proof.sn
                || previous_proof.genesis_governance_version
                    != new_proof.genesis_governance_version
                || previous_proof.namespace != new_proof.namespace
                || previous_proof.name != new_proof.name
                || previous_proof.subject_id != new_proof.subject_id
                || previous_proof.schema_id != new_proof.schema_id
                || previous_proof.governance_id != new_proof.governance_id
            {
                error!("");
                return Err(Error::Validation(format!("There are fields that do not match in the comparison of the previous validation proof and the new proof.")));
            }
            // Validate the previous proof
            // If all validations are correct, we get the public keys of the validators
            let previous_signers: Result<HashSet<KeyIdentifier>, Error> =
                previous_validation_signatures
                    .into_iter()
                    .map(|signer_res| {
                        match signer_res {
                            // Signer response
                            SignersRes::Signature(signature) => {

                                if let Err(error) = signature.verify(&previous_proof) {
                                    return Err(Error::Signature(format!("An error occurred while validating the previous proof, {:?}", error)));
                                } else {
                                    Ok(signature.signer)    
                                }
                            }
                            // TimeOut response
                            SignersRes::TimeOut(time_out) => Ok(time_out.who),
                        }
                    })
                    .collect();
            let previous_signers = previous_signers?;

            // Governance path
            let governance_path = ActorPath::from(format!(
                "/user/node/{}",
                new_proof.governance_id
            ));
            // Governance actor.
            let governance_actor: Option<ActorRef<Governance>> =
                ctx.system().get_actor(&governance_path).await;

            let message;

            // If the governance version is the same, we ask the governance for the current validators, to check that they are all part of it.
            if previous_proof.governance_version == new_proof.governance_version
            {
                message = GovernanceCommand::GetSigners {
                    stage: RequestStage::Validate,
                    schema_id: new_proof.schema_id.clone(),
                    namespace: new_proof.namespace.clone(),
                }
            } else {
                // Cambiarla cuando haga el TODO
                message = GovernanceCommand::GetSigners {
                    stage: RequestStage::Validate,
                    schema_id: new_proof.schema_id.clone(),
                    namespace: new_proof.namespace.clone(),
                }
            }

            // We obtain the actor governance
            let response = if let Some(governance_actor) = governance_actor {
                // We ask a node
                let response = governance_actor.ask(message).await;
                match response {
                    Ok(response) => response,
                    Err(e) => {
                        return Err(Error::Actor(format!(
                            "Error when asking a governance {}",
                            e
                        )));
                    }
                }
            } else {
                return Err(Error::Actor(format!("The governance actor was not found in the expected path /user/node/{}",  new_proof.governance_id)));
            };

            // If the governance version is the same, we ask the governance for the current validators, to check that they are all part of it.
            if previous_proof.governance_version == new_proof.governance_version
            {
                // preguntar a la governanza por los actuales validadores, comprobar que todos forman parte de ella.
                let actual_signers = match response {
                    GovernanceResponse::Signers(signers) => signers,
                    _ => return Err(Error::Actor(format!("An unexpected response has been received from governance actor")))
                };

                if actual_signers != previous_signers {
                    return Err(Error::Validation(format!("The previous event received validations from validators who are not part of governance.")));
                }
            } else {
                // TODO: Si la versión de la governanza es -1, solicitarle a la governanza los validadores de esa versión
            }

            Ok(previous_proof.subject_public_key.clone())

        // Genesis event, it is first proof
        } else {
            Ok(new_proof.subject_public_key.clone())
        }
    }

    fn is_finished(&self) -> bool {
        self.finish
    }

    fn set_finished(&mut self, value: bool) {
        self.finish = value;
    }
}

#[derive(Debug, Clone)]
pub enum ValidatorCommand {
    LocalValidation(ValidationReq, KeyIdentifier),
    NetworkValidation(DigestIdentifier, ValidationReq, KeyIdentifier),
    NetworkResponse,
    NetworkRequest(ValidationReq),
}

impl Message for ValidatorCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorEvent {
    AllTryHaveBeenMade,
    ReTry,
}

impl Event for ValidatorEvent {}

#[derive(Debug, Clone)]
pub enum ValidatorResponse {
    None,
}

impl Response for ValidatorResponse {}

#[async_trait]
impl Actor for Validator {
    type Event = ValidatorEvent;
    type Message = ValidatorCommand;
    type Response = ValidatorResponse;
}

#[async_trait]
impl Handler<Validator> for Validator {
    async fn handle_message(
        &mut self,
        msg: ValidatorCommand,
        ctx: &mut ActorContext<Validator>,
    ) -> Result<ValidatorResponse, ActorError> {
        match msg {
            ValidatorCommand::LocalValidation(validation_req, key_identifier) => {
                // Validate event
                let validation = match self.validation_event(ctx,validation_req).await {
                    Ok(validation) => ValidationCommand::Response(ValidationResponse::Response(ValidationRes::Signature(validation))),
                    Err(e) => {
                        // Log con el error.
                        ValidationCommand::Response(ValidationResponse::Response(ValidationRes::Error(key_identifier)))
                    }
                };
                
                // Validation path.
                let validation_path = ctx.path().parent();

                // Validation actor.
                let validation_actor: Option<ActorRef<Validation>> =
                    ctx.system().get_actor(&validation_path).await;
                
                // Send response of validation to parent
                if let Some(validation_actor) = validation_actor {
                    if let Err(e) = validation_actor.tell(validation).await {
                        return Err(e);
                    }
                } else {
                    // Can not obtain parent actor
                    return Err(ActorError::Exists(validation_path));
                }

                Ok(ValidatorResponse::None)
            }
            ValidatorCommand::NetworkValidation(
                request_id,
                validation_req,
                key_identifier,
            ) => {
                // Lanzar evento donde lanzar los retrys
                Ok(ValidatorResponse::None)
            }
            ValidatorCommand::NetworkResponse => {
                // Recibir respuesta de la network, darle la respuesta al padre, sea un fallo o no.
                Ok(ValidatorResponse::None)
            }
            ValidatorCommand::NetworkRequest(validation_req) => {
                // Validar y devolver la respuesta al helper, no a Validation.
                Ok(ValidatorResponse::None)
            }
        }
    }

    async fn on_event(
        &mut self,
        event: ValidatorEvent,
        ctx: &mut ActorContext<Validator>,
    ) {
        match event {
            ValidatorEvent::AllTryHaveBeenMade => {
                // Si llega este evento y is_finished omitir (la respuesta llegó después de terminar los eventos)
                // Si no está is_finished mandar un mensaje al padre con que No se pudo completar la validación para este actor, se supone que se da por validado.
            }
            ValidatorEvent::ReTry => {
                // Lanzar retrys (bloqueante)
            }
        }
    }
}

#[async_trait]
impl Retry for Validator {
    type Child = RetryValidator;

    async fn child(
        &self,
        ctx: &mut ActorContext<Validator>,
    ) -> Result<ActorRef<RetryValidator>, ActorError> {
        ctx.create_child("network", RetryValidator {}).await
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
            while retries < retry_strategy.max_retries() && !self.is_finished()
            {
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
            if self.is_finished() {
                // LLegó respuesta se abortan los intentos.
                Ok(ValidatorResponse::None)
            } else {
                error!("Max retries with actor {} reached.", path);
                // emitir evento de que todos los intentos fueron realizados
                Err(ActorError::Functional(format!(
                    "Max retries with actor {} reached.",
                    path
                )))
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
pub struct RetryValidator {}

#[derive(Debug, Clone)]
pub struct RetryValidatorCommand {}

impl Message for RetryValidatorCommand {}

#[async_trait]
impl Actor for RetryValidator {
    type Event = ValidatorEvent;
    type Message = RetryValidatorCommand;
    type Response = ValidatorResponse;
}

#[async_trait]
impl Handler<RetryValidator> for RetryValidator {
    async fn handle_message(
        &mut self,
        msg: RetryValidatorCommand,
        ctx: &mut ActorContext<RetryValidator>,
    ) -> Result<ValidatorResponse, ActorError> {
        // Enviar mensaje a la network
        Ok(ValidatorResponse::None)
    }
}
