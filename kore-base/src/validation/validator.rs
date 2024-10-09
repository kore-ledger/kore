// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::HashSet, time::Duration};

use crate::{
    governance::{model::Roles, Governance, RequestStage},
    helpers::network::{intermediary::Intermediary, NetworkMessage},
    model::{
        common::get_gov,
        network::{RetryNetwork, TimeOutResponse},
        signature::Signature,
        SignTypesNode, TimeStamp,
    },
    node::{self, Node, NodeMessage, NodeResponse},
    subject::{SubjectMessage, SubjectResponse},
    Error, Signed, Subject,
};

use super::{
    proof::{EventProof, ValidationProof},
    request::{SignersRes, ValidationReq},
    response::ValidationRes,
    Validation, ValidationMessage, ValidationResponse,
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
    FixedIntervalStrategy, Handler, Message, Response, RetryActor,
    RetryMessage, RetryStrategy, Strategy,
};

use tracing::{debug, error};

/// A struct representing a validator actor.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    request_id: String,
    node: KeyIdentifier,
}

impl Validator {
    pub fn new(request_id: String, node: KeyIdentifier) -> Self {
        Validator { request_id, node }
    }

    fn check_event_proof(
        &self,
        proof: &ValidationProof,
        subject_signature: &Signature,
        previous_proof: &Option<ValidationProof>,
    ) -> Result<(), Error> {
        let previous_proof = if let Some(previous_proof) = previous_proof {
            previous_proof
        } else {
            if proof.event != EventProof::Create {
                // Error
                todo!()
            }
            return Ok(());
        };

        let transfer_event = EventProof::Transfer {
            new_owner: KeyIdentifier::default(),
        };

        match proof.event.clone() {
            EventProof::Create => {
                // Error
                todo!()
            }
            EventProof::Fact => {
                if previous_proof.event == EventProof::EOL
                    || previous_proof.event == transfer_event
                {
                    //Error
                    todo!()
                }
            }
            EventProof::Transfer { new_owner } => {
                if previous_proof.event == EventProof::EOL
                    || previous_proof.event == transfer_event
                {
                    //Error
                    todo!()
                }
            }
            EventProof::Confirm => {
                if let EventProof::Transfer { new_owner } =
                    previous_proof.event.clone()
                {
                    if new_owner == subject_signature.signer {
                        return Ok(());
                    }
                }
                //Error
                todo!()
            }
            EventProof::EOL => {
                if previous_proof.event == EventProof::EOL
                    || previous_proof.event == transfer_event
                {
                    //Error
                    todo!()
                }
            }
        };

        if previous_proof.event != EventProof::Confirm {
            if previous_proof.subject_public_key
                != subject_signature.signer.clone()
            {
                // Error,
                todo!()
            }
        }

        Ok(())
    }

    // TODO si es un nuevo validador va a necesitar la prueba anterior y las firmas.
    async fn validation(
        &self,
        ctx: &mut ActorContext<Validator>,
        validation_req: ValidationReq,
    ) -> Result<Signature, Error> {
        // Obtain gov_version
        let actual_gov_version: u64 = if validation_req.proof.schema_id
            == "governance"
            && validation_req.proof.sn == 0
        {
            0
        // ask the government for its version
        } else {
            let governance_id =
                if validation_req.proof.schema_id == "governance" {
                    validation_req.proof.subject_id.clone()
                } else {
                    validation_req.proof.governance_id.clone()
                };

            get_gov(ctx, governance_id).await?.get_version()
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
        validation_req
            .subject_signature
            .verify(&validation_req.proof)?;

        self.check_event_proof(
            &validation_req.proof,
            &validation_req.subject_signature,
            &validation_req.previous_proof,
        )?;

        self.check_proofs(
            ctx,
            &validation_req.proof,
            validation_req.previous_proof,
            validation_req.prev_event_validation_response,
        )
        .await?;

        // Node path.
        let node_path = ActorPath::from("/user/node");
        // Node actor.
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        // We obtain the actor node
        let response = if let Some(node_actor) = node_actor {
            // We ask a node
            let response = node_actor
                .ask(NodeMessage::SignRequest(SignTypesNode::Validation(
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
            return Err(Error::Actor(
                "The node actor was not found in the expected path /user/node"
                    .to_owned(),
            ));
        };

        // We handle the possible responses of node
        match response {
            NodeResponse::SignRequest(sign) => Ok(sign),
            NodeResponse::Error(error) => Err(Error::Actor(format!(
                "The node encountered problems when signing the proof: {}",
                error
            ))),
            _ => Err(Error::Actor(
                "An unexpected response has been received from node actor"
                    .to_owned(),
            )),
        }
    }

    async fn check_proofs(
        &self,
        ctx: &mut ActorContext<Validator>,
        new_proof: &ValidationProof,
        previous_proof: Option<ValidationProof>,
        previous_validation_signatures: Vec<SignersRes>,
    ) -> Result<(), Error> {
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
                return Err(Error::Validation("There are fields that do not match in the comparison of the previous validation proof and the new proof.".to_owned()));
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
                                    Err(Error::Signature(format!("An error occurred while validating the previous proof, {:?}", error)))
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
            // Get validation signers
            let governance_id = if new_proof.schema_id == "governance" {
                new_proof.subject_id.clone()
            } else {
                new_proof.governance_id.clone()
            };

            let actual_signers =
                get_gov(ctx, governance_id).await?.get_signers(
                    Roles::VALIDATOR,
                    &new_proof.schema_id,
                    new_proof.namespace.clone(),
                );

            // If the governance version is the same, we ask the governance for the current validators, to check that they are all part of it.
            if previous_proof.governance_version == new_proof.governance_version
            {
                if actual_signers != previous_signers {
                    return Err(Error::Validation("The previous event received validations from validators who are not part of governance.".to_owned()));
                }
            } else {
                // TODO: Si la versión de la governanza es -1, solicitarle a la governanza los validadores de esa versión
            }
            Ok(())

        // Genesis event, it is first proof
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub enum ValidatorMessage {
    LocalValidation {
        validation_req: ValidationReq,
        our_key: KeyIdentifier,
    },
    NetworkValidation {
        request_id: String,
        validation_req: Signed<ValidationReq>,
        schema: String,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        validation_res: Signed<ValidationRes>,
        request_id: String,
    },
    NetworkRequest {
        validation_req: Signed<ValidationReq>,
        info: ComunicateInfo,
    },
}

impl Message for ValidatorMessage {}

#[async_trait]
impl Actor for Validator {
    type Event = ();
    type Message = ValidatorMessage;
    type Response = ();
}

#[async_trait]
impl Handler<Validator> for Validator {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: ValidatorMessage,
        ctx: &mut ActorContext<Validator>,
    ) -> Result<(), ActorError> {
        match msg {
            ValidatorMessage::LocalValidation {
                validation_req,
                our_key,
            } => {
                // Validate event
                let validation = match self
                    .validation(ctx, validation_req)
                    .await
                {
                    Ok(validation) => ValidationMessage::Response {
                        validation_res: ValidationRes::Signature(validation),
                        sender: our_key,
                    },
                    Err(e) => {
                        // Log con el error. TODO
                        ValidationMessage::Response {
                            validation_res: ValidationRes::Error(format!(
                                "{}",
                                e
                            )),
                            sender: our_key,
                        }
                    }
                };

                // Validation path.
                let validation_path = ctx.path().parent();

                // Validation actor.
                let validation_actor: Option<ActorRef<Validation>> =
                    ctx.system().get_actor(&validation_path).await;

                // Send response of validation to parent
                if let Some(validation_actor) = validation_actor {
                    validation_actor.tell(validation).await?
                } else {
                    // Can not obtain parent actor
                    return Err(ActorError::Exists(validation_path));
                }

                ctx.stop().await;
            }
            ValidatorMessage::NetworkValidation {
                request_id,
                validation_req,
                schema,
                node_key,
                our_key,
            } => {
                let reciver_actor = if schema == "governance" {
                    format!(
                        "/user/node/{}/validator",
                        validation_req.content.proof.subject_id
                    )
                } else {
                    format!(
                        "/user/node/{}/{}_validation",
                        validation_req.content.proof.governance_id, schema
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
                    message: ActorMessage::ValidationReq {
                        req: validation_req,
                    },
                };

                let target = RetryNetwork::default();

                let strategy = Strategy::FixedInterval(
                    FixedIntervalStrategy::new(3, Duration::from_secs(3)),
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

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    todo!()
                };
            }
            ValidatorMessage::NetworkResponse {
                validation_res,
                request_id,
            } => {
                if request_id == self.request_id {
                    if self.node != validation_res.signature.signer {
                        // Nos llegó a una validación de un nodo incorrecto!
                        todo!()
                    }

                    if let Err(e) = validation_res.verify() {
                        // Hay error criptográfico en la respuesta
                        todo!()
                    }

                    // Validation path.
                    let validation_path = ctx.path().parent();

                    // Validation actor.
                    let validation_actor: Option<ActorRef<Validation>> =
                        ctx.system().get_actor(&validation_path).await;

                    if let Some(validation_actor) = validation_actor {
                        if let Err(e) = validation_actor
                            .tell(ValidationMessage::Response {
                                validation_res: validation_res.content,
                                sender: self.node.clone(),
                            })
                            .await
                        {
                            // TODO error, no se puede enviar la response. Parar
                        }
                    } else {
                        // TODO no se puede obtener validation! Parar.
                        // Can not obtain parent actor
                    }

                    let retry = if let Some(retry) =
                        ctx.get_child::<RetryActor<RetryNetwork>>("retry").await
                    {
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
            }
            ValidatorMessage::NetworkRequest {
                validation_req,
                info,
            } => {
                // TODO lo primero que hay que hacer es comprobar la versión de la governanza,
                if info.schema == "governance" {
                    // Aquí hay que comprobar que el owner del subject es el que envía la req.
                    let subject_path = ActorPath::from(format!(
                        "/user/node/{}",
                        validation_req.content.proof.subject_id.clone()
                    ));
                    let subject_actor: Option<ActorRef<Subject>> =
                        ctx.system().get_actor(&subject_path).await;

                    // We obtain the validator
                    let response = if let Some(subject_actor) = subject_actor {
                        match subject_actor.ask(SubjectMessage::GetOwner).await
                        {
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

                    if subject_owner != validation_req.signature.signer {
                        // Error nos llegó una validation req de un nodo el cual no es el dueño
                        todo!()
                    }

                    if let Err(e) = validation_req.verify() {
                        // Hay errores criptográficos
                        todo!()
                    }
                }

                // Llegados a este punto se ha verificado que la req es del owner del sujeto y está todo correcto.

                // Validar y devolver la respuesta al helper, no a Validation. Nos llegó por la network la validación.
                // Sacar el Helper aquí
                let helper: Option<Intermediary> =
                    ctx.system().get_helper("NetworkIntermediary").await;
                let mut helper = if let Some(helper) = helper {
                    helper
                } else {
                    // TODO error no se puede acceder al helper, cambiar este error. este comando se envía con Tell, por lo tanto el error hay que propagarlo hacia arriba directamente, no con
                    // return Err(ActorError::Get("Error".to_owned()))
                    return Err(ActorError::NotHelper);
                };

                let validation = match self
                    .validation(ctx, validation_req.content.clone())
                    .await
                {
                    Ok(validation) => ValidationRes::Signature(validation),
                    Err(e) => {
                        // Log con el error. TODO
                        ValidationRes::Error(format!("{}", e))
                    }
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    reciver_actor: format!(
                        "/user/node/{}/validation/{}",
                        validation_req.content.proof.subject_id,
                        info.reciver.clone()
                    ),
                    schema: info.schema.clone(),
                };

                // Aquí tiene que firmar el nodo la respuesta.
                let node_path = ActorPath::from("/user/node");
                let node_actor: Option<ActorRef<Node>> =
                    ctx.system().get_actor(&node_path).await;

                // We obtain the validator
                let node_response = if let Some(node_actor) = node_actor {
                    match node_actor
                        .ask(NodeMessage::SignRequest(
                            SignTypesNode::ValidationRes(validation.clone()),
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

                let signed_response: Signed<ValidationRes> = Signed {
                    content: validation,
                    signature,
                };

                if let Err(e) = helper
                    .send_command(network::CommandHelper::SendMessage {
                        message: NetworkMessage {
                            info: new_info,
                            message: ActorMessage::ValidationRes {
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
        ctx: &mut ActorContext<Validator>,
    ) {
        if let ActorError::Functional(error) = error {
            if &error == "Max retries reached." {
                let validation_path = ctx.path().parent();

                // Validation actor.
                let validation_actor: Option<ActorRef<Validation>> =
                    ctx.system().get_actor(&validation_path).await;

                if let Some(validation_actor) = validation_actor {
                    if let Err(e) = validation_actor
                        .tell(ValidationMessage::Response {
                            validation_res: ValidationRes::TimeOut(
                                TimeOutResponse {
                                    re_trys: 3,
                                    timestamp: TimeStamp::now(),
                                    who: self.node.clone(),
                                },
                            ),
                            sender: self.node.clone(),
                        })
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
    }
}
