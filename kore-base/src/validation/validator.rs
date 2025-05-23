// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::HashSet, time::Duration};

use crate::{
    Signed,
    governance::{Governance, model::SignersType},
    helpers::network::{NetworkMessage, intermediary::Intermediary},
    model::{
        SignTypesNode, TimeStamp,
        common::{
            UpdateData, emit_fail, get_gov, get_metadata, get_sign,
            update_ledger_network,
        },
        event::ProtocolsSignatures,
        network::{RetryNetwork, TimeOutResponse},
    },
};

use super::{
    Validation, ValidationMessage, proof::ValidationProof,
    request::ValidationReq, response::ValidationRes,
};

use crate::helpers::network::ActorMessage;

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};

use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    FixedIntervalStrategy, Handler, Message, RetryActor, RetryMessage,
    Strategy,
};
use tracing::{error, warn};

const TARGET_VALIDATOR: &str = "Kore-Validation-Validator";

/// A struct representing a validator actor.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    request_id: String,
    version: u64,
    node: KeyIdentifier,
}

impl Validator {
    pub fn new(request_id: String, version: u64, node: KeyIdentifier) -> Self {
        Validator {
            request_id,
            version,
            node,
        }
    }

    async fn check_governance(
        &self,
        ctx: &mut ActorContext<Validator>,
        governance_id: DigestIdentifier,
        gov_version: u64,
        our_node: KeyIdentifier,
    ) -> Result<bool, ActorError> {
        let governance_string = governance_id.to_string();
        let metadata = get_metadata(ctx, &governance_string).await?;
        let governance = match Governance::try_from(metadata.properties.clone())
        {
            Ok(gov) => gov,
            Err(e) => {
                let e = format!(
                    "can not convert governance from properties: {}",
                    e
                );
                return Err(ActorError::FunctionalFail(e));
            }
        };

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

    async fn validation(
        &self,
        ctx: &mut ActorContext<Validator>,
        validation_req: ValidationReq,
    ) -> Result<ValidationRes, ActorError> {
        if let Some(reboot) = self
            .check_proofs(
                ctx,
                &validation_req.proof,
                validation_req.previous_proof,
                validation_req.prev_event_validation_response,
            )
            .await?
        {
            return Ok(reboot);
        };

        let signature = get_sign(
            ctx,
            SignTypesNode::Validation(Box::new(validation_req.proof)),
        )
        .await?;

        Ok(ValidationRes::Signature(signature))
    }

    async fn check_proofs(
        &self,
        ctx: &mut ActorContext<Validator>,
        new_proof: &ValidationProof,
        previous_proof: Option<ValidationProof>,
        previous_validation_signatures: Vec<ProtocolsSignatures>,
    ) -> Result<Option<ValidationRes>, ActorError> {
        // Not genesis event
        if let Some(previous_proof) = previous_proof {
            if new_proof.error_not_create(&previous_proof) {
                return Err(ActorError::Functional("There are fields that do not match in the comparison of the previous validation proof and the new proof.".to_owned()));
            }
            // Get validation signers
            let governance_id = if new_proof.schema_id == "governance" {
                new_proof.subject_id.clone()
            } else {
                new_proof.governance_id.clone()
            };

            let gov = get_gov(ctx, &governance_id.to_string()).await?;

            // If the governance version is the same, we ask the governance for the current validators, to check that they are all part of it.
            if previous_proof.governance_version == gov.version {
                let actual_signers = gov
                    .get_signers(
                        SignersType::Validator,
                        &new_proof.schema_id,
                        new_proof.namespace.clone(),
                    )
                    .0;

                // Validate the previous proof
                // If all validations are correct, we get the public keys of the validators
                let previous_signers: Result<HashSet<KeyIdentifier>, ActorError> =
                previous_validation_signatures
                    .into_iter()
                    .map(|signer_res| {
                        match signer_res {
                            // Signer response
                            ProtocolsSignatures::Signature(signature) => {

                                if let Err(error) = signature.verify(&previous_proof) {
                                    Err(ActorError::Functional(format!("An error occurred while validating the previous proof, {:?}", error)))
                                } else {
                                    Ok(signature.signer)
                                }
                            }
                            // TimeOut response
                            ProtocolsSignatures::TimeOut(time_out) => Ok(time_out.who),
                        }
                    })
                    .collect();
                let previous_signers = previous_signers?;

                if actual_signers != previous_signers {
                    return Err(ActorError::Functional("The previous event received validations from validators who are not part of governance.".to_owned()));
                }
            }
        // Genesis event, it is first proof
        } else if new_proof.error_create() {
            return Err(ActorError::Functional(
                "There are incorrect fields in the validation test".to_owned(),
            ));
        }

        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub enum ValidatorMessage {
    LocalValidation {
        validation_req: ValidationReq,
        our_key: KeyIdentifier,
    },
    NetworkValidation {
        validation_req: Signed<ValidationReq>,
        schema_id: String,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        validation_res: Signed<ValidationRes>,
        request_id: String,
        version: u64,
    },
    NetworkRequest {
        validation_req: Box<Signed<ValidationReq>>,
        info: ComunicateInfo,
        schema_id: String,
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
        _sender: ActorPath,
        msg: ValidatorMessage,
        ctx: &mut ActorContext<Validator>,
    ) -> Result<(), ActorError> {
        match msg {
            ValidatorMessage::LocalValidation {
                validation_req,
                our_key,
            } => {
                // Validate event
                let validation_res =
                    match self.validation(ctx, validation_req).await {
                        Ok(res) => res,
                        Err(e) => {
                            if let ActorError::Functional(_) = e {
                                warn!(
                                    TARGET_VALIDATOR,
                                    "LocalValidation, validation error: {}", e
                                );
                                ValidationRes::Error(e.to_string())
                            } else {
                                error!(
                                    TARGET_VALIDATOR,
                                    "LocalValidation, validation error: {}", e
                                );
                                return Err(emit_fail(ctx, e).await);
                            }
                        }
                    };

                let validation = ValidationMessage::Response {
                    validation_res,
                    sender: our_key,
                };

                // Validation path.
                let validation_path = ctx.path().parent();

                // Validation actor.
                let validation_actor: Option<ActorRef<Validation>> =
                    ctx.system().get_actor(&validation_path).await;

                // Send response of validation to parent
                if let Some(validation_actor) = validation_actor {
                    if let Err(e) = validation_actor.tell(validation).await {
                        error!(
                            TARGET_VALIDATOR,
                            "LocalValidation, can not send local validation to Validation actor: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    };
                } else {
                    error!(
                        TARGET_VALIDATOR,
                        "LocalValidation, can not obtain Validation actor"
                    );
                    let e = ActorError::Exists(validation_path);
                    return Err(emit_fail(ctx, e).await);
                }

                ctx.stop(None).await;
            }
            ValidatorMessage::NetworkValidation {
                validation_req,
                schema_id,
                node_key,
                our_key,
            } => {
                let reciver_actor = if schema_id == "governance" {
                    format!(
                        "/user/node/{}/validator",
                        validation_req.content.proof.subject_id
                    )
                } else {
                    format!(
                        "/user/node/{}/{}_validation",
                        validation_req.content.proof.governance_id, schema_id
                    )
                };

                // Lanzar evento donde lanzar los retrys
                let message = NetworkMessage {
                    info: ComunicateInfo {
                        request_id: self.request_id.to_owned(),
                        version: self.version,
                        sender: our_key,
                        reciver: node_key,
                        reciver_actor,
                    },
                    message: ActorMessage::ValidationReq {
                        req: Box::new(validation_req),
                        schema_id,
                    },
                };

                let target = RetryNetwork::default();

                let strategy = Strategy::FixedInterval(
                    FixedIntervalStrategy::new(3, Duration::from_secs(3)),
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
                            TARGET_VALIDATOR,
                            "NetworkValidation, can not obtain Retry actor: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                if let Err(e) = retry.tell(RetryMessage::Retry).await {
                    error!(
                        TARGET_VALIDATOR,
                        "NetworkValidation, can not send retry to Retry actor: {}",
                        e
                    );
                    return Err(emit_fail(ctx, e).await);
                };
            }
            ValidatorMessage::NetworkResponse {
                validation_res,
                request_id,
                version,
            } => {
                if request_id == self.request_id && version == self.version {
                    if self.node != validation_res.signature.signer {
                        warn!(
                            TARGET_VALIDATOR,
                            "NetworkResponse, invalid signer"
                        );
                        return Err(ActorError::Functional(
                            "Invalid signer".to_owned(),
                        ));
                    }

                    if let Err(e) = validation_res.verify() {
                        warn!(
                            TARGET_VALIDATOR,
                            "NetworkResponse, can not verify signature: {}", e
                        );
                        return Err(ActorError::Functional(format!(
                            "Can not verify signature: {}",
                            e
                        )));
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
                            error!(
                                TARGET_VALIDATOR,
                                "NetworkResponse, can not send response to Validation actor {}",
                                e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    } else {
                        let e = ActorError::NotFound(validation_path);
                        error!(
                            TARGET_VALIDATOR,
                            "NetworkResponse, can not obtain Validation actor {}",
                            e
                        );
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
                            warn!(
                                TARGET_VALIDATOR,
                                "NetworkResponse, can not end Retry actor: {}",
                                e
                            );
                            // Aquí me da igual, porque al parar este actor para el hijo
                            break 'retry;
                        };
                    }

                    ctx.stop(None).await;
                } else {
                    warn!(
                        TARGET_VALIDATOR,
                        "NetworkResponse, invalid request id"
                    );
                }
            }
            ValidatorMessage::NetworkRequest {
                validation_req,
                info,
                schema_id,
            } => {
                let info_subject_path =
                    ActorPath::from(info.reciver_actor.clone()).parent().key();
                let governance_id =
                    if validation_req.content.proof.governance_id.is_empty() {
                        validation_req.content.proof.subject_id.clone()
                    } else {
                        validation_req.content.proof.governance_id.clone()
                    };

                if info_subject_path != governance_id.to_string() {
                    let e = "We received an evaluation where the request indicates one subject but the info indicates another.";
                    error!(TARGET_VALIDATOR, "NetworkRequest, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                if let Err(e) = validation_req.verify() {
                    let e =
                        format!("Can not verify signature of request: {}", e);
                    error!(TARGET_VALIDATOR, "NetworkRequest, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                let helper: Option<Intermediary> =
                    ctx.system().get_helper("network").await;

                let Some(mut helper) = helper else {
                    let e = ActorError::NotHelper("network".to_owned());
                    error!(
                        TARGET_VALIDATOR,
                        "NetworkRequest, can not obtain network helper"
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                let reboot = match self
                    .check_governance(
                        ctx,
                        governance_id,
                        validation_req.content.proof.governance_version,
                        info.reciver.clone(),
                    )
                    .await
                {
                    Ok(reboot) => reboot,
                    Err(e) => {
                        if let ActorError::Functional(_) = e {
                            warn!(
                                TARGET_VALIDATOR,
                                "NetworkRequest, checking governance: {}", e
                            );
                            return Err(e);
                        } else {
                            error!(
                                TARGET_VALIDATOR,
                                "NetworkRequest, checking governance: {}", e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    }
                };

                let validation = if reboot {
                    ValidationRes::Reboot
                } else {
                    match self
                        .validation(ctx, validation_req.content.clone())
                        .await
                    {
                        Ok(res) => res,
                        Err(e) => {
                            if let ActorError::Functional(_) = e {
                                warn!(
                                    TARGET_VALIDATOR,
                                    "NetworkRequest, validation error: {}", e
                                );
                                ValidationRes::Error(e.to_string())
                            } else {
                                error!(
                                    TARGET_VALIDATOR,
                                    "NetworkRequest, validation error: {}", e
                                );
                                return Err(emit_fail(ctx, e).await);
                            }
                        }
                    }
                };

                let new_info = ComunicateInfo {
                    reciver: info.sender,
                    sender: info.reciver.clone(),
                    request_id: info.request_id,
                    version: info.version,
                    reciver_actor: format!(
                        "/user/node/{}/validation/{}",
                        validation_req.content.proof.subject_id,
                        info.reciver.clone()
                    ),
                };

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::ValidationRes(validation.clone()),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        error!(
                            TARGET_VALIDATOR,
                            "NetworkRequest, can not sign response: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
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
                    error!(
                        TARGET_VALIDATOR,
                        "NetworkRequest, can not send response to network: {}",
                        e
                    );
                    return Err(emit_fail(ctx, e).await);
                };

                if schema_id != "governance" {
                    ctx.stop(None).await;
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
        match error {
            ActorError::ReTry => {
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
                        error!(
                            TARGET_VALIDATOR,
                            "OnChildError, can not send response to Validation actor: {}",
                            e
                        );
                        emit_fail(ctx, e).await;
                    }
                } else {
                    let e = ActorError::NotFound(validation_path);
                    error!(
                        TARGET_VALIDATOR,
                        "OnChildError, can not obtain Validation actor: {}", e
                    );
                    emit_fail(ctx, e).await;
                }
                ctx.stop(None).await;
            }
            _ => {
                error!(TARGET_VALIDATOR, "OnChildError, unexpected error");
            }
        };
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Validator>,
    ) -> ChildAction {
        error!(TARGET_VALIDATOR, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}
