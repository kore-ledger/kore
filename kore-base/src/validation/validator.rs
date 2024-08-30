// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::HashSet, fmt::format, str::FromStr, time::Duration};

use crate::{
    governance::{Governance, RequestStage},
    helpers::network::{intermediary::Intermediary, NetworkMessage},
    model::{signature::Signature, SignTypes, TimeStamp},
    node::{Node, NodeMessage, NodeResponse},
    subject::{SubjectCommand, SubjectResponse},
    Error, Subject,
};

use super::{
    proof::ValidationProof,
    request::{SignersRes, ValidationReq},
    response::{ValidationError, ValidationRes, ValidationTimeOut},
    Validation, ValidationCommand, ValidationResponse,
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

/// A struct representing a validator actor.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    request_id: String,
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
                "The governance actor was not found in the expected path /user/node/{}",
                validation_req.proof.governance_id
            )));
            };

            match response {
                SubjectResponse::Governance(gov) => gov.version(),
                SubjectResponse::Error(error) => {
                    return Err(Error::Actor(format!("The subject encountered problems when getting governance: {}",error)));
                }
                _ => {
                    return Err(Error::Actor(format!(
                    "An unexpected response has been received from node actor"
                )))
                }
            }
        };
        match actual_gov_version.cmp(&validation_req.proof.governance_version) {
            std::cmp::Ordering::Equal => {
                // If it is the same it means that we have the latest version of governance, we are up to date.
            }
            std::cmp::Ordering::Greater => {
                // It is impossible to have a greater version of governance than the owner of the governance himself.
                // The only possibility is that it is an old validation request.
                // Hay que hacerlo TODO
            }
            std::cmp::Ordering::Less => {
                // Stop validation process, we need to update governance, we are out of date.
                // Hay que hacerlo TODO
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
            NodeResponse::SignRequest(sign) => Ok(sign),
            NodeResponse::Error(error) => Err(Error::Actor(format!(
                "The node encountered problems when signing the proof: {}",
                error
            ))),
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

            // TODO previamente se obtiene la governanza, ver si podemos refactorizar para no tener que volver a pedirla
            // Governance path
            let governance_path = ActorPath::from(format!(
                "/user/node/{}",
                new_proof.governance_id
            ));

            // Governance actor.
            let governance_actor: Option<ActorRef<Subject>> =
                ctx.system().get_actor(&governance_path).await;

            // We obtain the actor governance
            let response = if let Some(governance_actor) = governance_actor {
                // We ask a governance
                // TODO si previous_proof.governance_version == new_proof.governance_version pedimos la governanza, sino tenemos que obtener la versión anterior de governanza
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
                    "The governance actor was not found in the expected path /user/node/{}",
                    new_proof.governance_id
                )));
            };

            let actual_signers = match response {
                SubjectResponse::Governance(gov) => gov.get_signers(
                    RequestStage::Validate,
                    &new_proof.schema_id,
                    new_proof.namespace.clone(),
                ),
                SubjectResponse::Error(error) => {
                    return Err(Error::Actor(format!("The subject encountered problems when getting governance: {}",error)));
                }
                _ => {
                    return Err(Error::Actor(format!(
                    "An unexpected response has been received from node actor"
                    )))
                }
            };

            // If the governance version is the same, we ask the governance for the current validators, to check that they are all part of it.
            if previous_proof.governance_version == new_proof.governance_version
            {
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
}

#[derive(Debug, Clone)]
pub enum ValidatorCommand {
    LocalValidation {
        validation_req: ValidationReq,
        our_key: KeyIdentifier,
    },
    NetworkValidation {
        request_id: String,
        validation_req: ValidationReq,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        validation_res: ValidationRes,
        request_id: String,
    },
    NetworkRequest {
        validation_req: ValidationReq,
        info: ComunicateInfo,
    },
}

impl Message for ValidatorCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidatorEvent {
    AllTryHaveBeenMade { node_key: KeyIdentifier },
    ReTry(NetworkMessage),
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
            ValidatorCommand::LocalValidation {
                validation_req,
                our_key,
            } => {
                // Validate event
                let validation =
                    match self.validation_event(ctx, validation_req).await {
                        Ok(validation) => ValidationCommand::Response(
                            ValidationRes::Signature(validation),
                        ),
                        Err(e) => {
                            // Log con el error. TODO
                            ValidationCommand::Response(ValidationRes::Error(
                                ValidationError {
                                    who: our_key,
                                    error: format!("{}", e),
                                },
                            ))
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
            ValidatorCommand::NetworkValidation {
                request_id,
                validation_req,
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
                            "/user/node/{}/validator",
                            validation_req.subject_signature.signer
                        ),
                    },
                    message: ActorMessage::ValidationReq(validation_req),
                };

                if let Err(e) = ctx.event(ValidatorEvent::ReTry(message)).await
                {
                    // TODO, error al crear evento, propagar hacia arriba
                };
                Ok(ValidatorResponse::None)
            }
            ValidatorCommand::NetworkResponse {
                validation_res,
                request_id,
            } => {
                if request_id == self.request_id {
                    // Validation path.
                    let validation_path = ctx.path().parent();

                    // Validation actor.
                    let validation_actor: Option<ActorRef<Validation>> =
                        ctx.system().get_actor(&validation_path).await;

                    if let Some(validation_actor) = validation_actor {
                        if let Err(e) = validation_actor
                            .tell(ValidationCommand::Response(validation_res))
                            .await
                        {
                            // TODO error, no se puede enviar la response. Parar
                        }
                    } else {
                        // TODO no se puede obtener validation! Parar.
                        // Can not obtain parent actor
                    }

                    self.finish = true;
                } else {
                    // TODO llegó una respuesta con una request_id que no es la que estamos esperando, no es válido.
                }

                Ok(ValidatorResponse::None)
            }
            ValidatorCommand::NetworkRequest {
                validation_req,
                info,
            } => {
                // Validar y devolver la respuesta al helper, no a Validation. Nos llegó por la network la validación.
                // Sacar el Helper aquí
                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    reciver_actor: format!(
                        "/user/node/{}/validation/{}",
                        validation_req.subject_signature.signer,
                        info.reciver.clone()
                    ),
                };
                let helper: Option<Intermediary> =
                    ctx.system().get_helper("NetworkIntermediary").await;
                let mut helper = if let Some(helper) = helper {
                    helper
                } else {
                    // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                    // return Err(ActorError::Get("Error".to_owned()))
                    return Err(ActorError::NotHelper);
                };

                let response = match self
                    .validation_event(ctx, validation_req.clone())
                    .await
                {
                    Ok(validation) => ValidationRes::Signature(validation),
                    Err(e) => {
                        // Log con el error. TODO
                        ValidationRes::Error(ValidationError {
                            who: validation_req.subject_signature.signer,
                            error: format!("{}", e),
                        })
                    }
                };

                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::ValidationRes(response),
                        },
                    })
                    .await
                {
                    // error al enviar mensaje, propagar hacia arriba TODO
                };

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
            ValidatorEvent::AllTryHaveBeenMade { node_key } => {
                if !self.finish {
                    // Validation path.
                    let validation_path = ctx.path().parent();

                    // Validation actor.
                    let validation_actor: Option<ActorRef<Validation>> =
                        ctx.system().get_actor(&validation_path).await;

                    if let Some(validation_actor) = validation_actor {
                        if let Err(e) = validation_actor
                            .tell(ValidationCommand::Response(
                                ValidationRes::TimeOut(ValidationTimeOut {
                                    re_trys: 3,
                                    timestamp: TimeStamp::now(),
                                    who: node_key,
                                }),
                            ))
                            .await
                        {
                            // TODO error, no se puede enviar la response
                            // return Err(e);
                        }
                    } else {
                        // TODO no se puede obtener validation! Parar.
                        // Can not obtain parent actor
                        // return Err(ActorError::Exists(validation_path));
                    }
                }
            }
            ValidatorEvent::ReTry(message) => {
                let path = ctx.path().clone() / "network";
                // TODO analizar la estrategia.
                let mut strategy = Strategy::FixedInterval(
                    FixedIntervalStrategy::new(3, Duration::from_secs(1)),
                );

                if let Err(e) = self
                    .apply_retries(ctx, path, &mut strategy, message.clone())
                    .await
                {
                    match e {
                        ActorError::ReTry => {
                            if let Err(e) = ctx
                                .event(ValidatorEvent::AllTryHaveBeenMade {
                                    node_key: message.info.reciver,
                                })
                                .await
                            {
                                // TODO, error al crear evento, propagar hacia arriba
                            };
                        }
                        ActorError::Functional(e) => {
                            // TODO, interno al hacer retry, propagar hacia arriba
                        }
                        _ => {
                            // No puede llegar ningún tipo de error más
                        }
                    }
                };
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
                Ok(ValidatorResponse::None)
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
pub struct RetryValidator {}

impl Message for NetworkMessage {}

#[async_trait]
impl Actor for RetryValidator {
    type Event = ValidatorEvent;
    type Message = NetworkMessage;
    type Response = ValidatorResponse;
}

#[async_trait]
impl Handler<RetryValidator> for RetryValidator {
    async fn handle_message(
        &mut self,
        msg: NetworkMessage,
        ctx: &mut ActorContext<RetryValidator>,
    ) -> Result<ValidatorResponse, ActorError> {
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
        Ok(ValidatorResponse::None)
    }
}
