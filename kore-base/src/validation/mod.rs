// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Validation module.
//!

pub mod proof;
pub mod request;
pub mod response;
pub mod schema;
pub mod validator;

use crate::{
    auth::WitnessesAuth, governance::{model::ProtocolTypes, Quorum}, model::{
        common::{
            emit_fail, get_sign, get_signers_quorum_gov_version,
            send_reboot_to_req, try_to_update,
        }, event::{ProofEvent, ProtocolsSignatures}, signature::Signed, SignTypesNode
    }, request::manager::{RequestManager, RequestManagerMessage}, subject::Metadata
};
use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    Handler, Message,
};

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use proof::ValidationProof;
use request::ValidationReq;
use response::ValidationRes;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use validator::{Validator, ValidatorMessage};

use std::collections::HashSet;

const TARGET_VALIDATION: &str = "Kore-Validation";

/// A struct for passing validation information.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationInfo {
    pub metadata: Metadata,
    pub event_proof: Signed<ProofEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Validation {
    node_key: KeyIdentifier,
    // Quorum
    quorum: Quorum,
    // Validators
    validators: HashSet<KeyIdentifier>,
    // Actual responses
    validators_response: Vec<ProtocolsSignatures>,

    validators_timeout: Vec<ProtocolsSignatures>,
    // Validators quantity
    validators_quantity: u32,

    actual_proof: ValidationProof,

    errors: String,

    valid_validation: bool,

    request_id: String,
    version: u64,

    reboot: bool,
}

impl Validation {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Validation {
            node_key,
            ..Default::default()
        }
    }

    async fn end_validators(&self, ctx: &mut ActorContext<Validation>) -> Result<(), ActorError> {
        for validator in self.validators.clone() {
            let child: Option<ActorRef<Validator>> =
                ctx.get_child(&validator.to_string()).await;
            if let Some(child) = child {
                child.ask_stop().await?;
            }
        }

        Ok(())
    }

    fn check_validator(&mut self, validator: KeyIdentifier) -> bool {
        self.validators.remove(&validator)
    }

    async fn create_validation_req(
        &self,
        validation_info: ValidationInfo,
        previous_proof: Option<ValidationProof>,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    ) -> Result<ValidationReq, ActorError> {
        let prev_evet_hash =
            if let Some(previous_proof) = previous_proof.clone() {
                previous_proof.event_hash
            } else {
                DigestIdentifier::default()
            };

        // Create proof from validation info
        let proof = ValidationProof::from_info(validation_info, prev_evet_hash)
            .map_err(|e| ActorError::FunctionalFail(e.to_string()))?;

        Ok(ValidationReq {
            proof: proof.clone(),
            previous_proof: previous_proof.clone(),
            prev_event_validation_response: prev_event_validation_response
                .clone(),
        })
    }

    async fn create_validators(
        &self,
        ctx: &mut ActorContext<Validation>,
        validation_req: Signed<ValidationReq>,
        schema: &str,
        signer: KeyIdentifier,
    ) -> Result<(), ActorError> {
        // Create Validator child
        let child = ctx
            .create_child(
                &format!("{}", signer),
                Validator::new(
                    self.request_id.to_owned(),
                    self.version,
                    signer.clone(),
                ),
            )
            .await;
        let validator_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(e),
        };

        // Check node_key
        let our_key = self.node_key.clone();
        // We are signer
        if signer == our_key {
            validator_actor
                .tell(ValidatorMessage::LocalValidation {
                    validation_req: validation_req.content,
                    our_key: signer,
                })
                .await?
        }
        // Other node is signer
        else {
            validator_actor
                .tell(ValidatorMessage::NetworkValidation {
                    validation_req,
                    node_key: signer,
                    our_key,
                    schema: schema.to_owned(),
                })
                .await?
        }

        Ok(())
    }

    async fn send_validation_to_req(
        &self,
        ctx: &mut ActorContext<Validation>,
        result: bool,
    ) -> Result<(), ActorError> {
        let mut error = self.errors.clone();
        if !result && error.is_empty() {
            let gov_id = if self.actual_proof.governance_id.is_empty() {
                self.actual_proof.subject_id.clone()
            } else {
                self.actual_proof.governance_id.clone()
            };
            "who: ALL, error: No validator was able to validate the event."
                .clone_into(&mut error);

            if self.validators_response.is_empty() {
                try_to_update(ctx, gov_id, WitnessesAuth::Witnesses).await?
            }
        }

        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        if let Some(req_actor) = req_actor {
            let mut signatures =  self.validators_response.clone();
            signatures.append(&mut self.validators_timeout.clone());
            
            req_actor
                .tell(RequestManagerMessage::ValidationRes {
                    result,
                    last_proof: Box::new(self.actual_proof.clone()),
                    signatures,
                    errors: error,
                })
                .await?
        } else {
            return Err(ActorError::NotFound(req_path));
        };

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ValidationMessage {
    Create {
        request_id: String,
        version: u64,
        info: ValidationInfo,
        last_proof: Box<Option<ValidationProof>>,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
    Response {
        validation_res: ValidationRes,
        sender: KeyIdentifier,
    },
}

impl Message for ValidationMessage {}

#[async_trait]
impl Actor for Validation {
    type Event = ();
    type Message = ValidationMessage;
    type Response = ();

    async fn pre_start(
        &mut self,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Ok(())
    }

    async fn pre_stop(
        &mut self,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Ok(())
    }
}

#[async_trait]
impl Handler<Validation> for Validation {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: ValidationMessage,
        ctx: &mut ActorContext<Validation>,
    ) -> Result<(), ActorError> {
        match msg {
            ValidationMessage::Create {
                request_id,
                info,
                version,
                last_proof,
                prev_event_validation_response,
            } => {
                let validation_req = match self
                    .create_validation_req(
                        info.clone(),
                        *last_proof,
                        prev_event_validation_response,
                    )
                    .await
                {
                    Ok(validation_req) => validation_req,
                    Err(e) => {
                        error!(
                            TARGET_VALIDATION,
                            "Create, can not create validation request: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };
                self.actual_proof = validation_req.proof.clone();

                // Get signers and quorum
                let (signers, quorum, _) = match get_signers_quorum_gov_version(
                    ctx,
                    &info.metadata.subject_id.to_string(),
                    &info.metadata.schema_id,
                    info.metadata.namespace.clone(),
                    ProtocolTypes::Validation,
                )
                .await
                {
                    Ok(signers_quorum) => signers_quorum,
                    Err(e) => {
                        error!(
                            TARGET_VALIDATION,
                            "Create, can not create obtain signers and quorum: {}",
                            e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                // Update quorum and validators
                self.valid_validation = false;
                self.errors = String::default();
                self.validators_response = vec![];
                self.quorum = quorum;
                self.validators.clone_from(&signers);
                self.validators_quantity = signers.len() as u32;
                self.request_id = request_id.clone();
                self.version = version;
                self.reboot = false;

                let signature = match get_sign(
                    ctx,
                    SignTypesNode::ValidationReq(Box::new(
                        validation_req.clone(),
                    )),
                )
                .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        error!(
                            TARGET_VALIDATION,
                            "Create, can not sign request: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    }
                };

                let signed_validation_req: Signed<ValidationReq> = Signed {
                    content: validation_req,
                    signature,
                };


                for signer in signers {
                    if let Err(e) =
                        self.create_validators(
                        ctx,
                        signed_validation_req.clone(),
                        &info.metadata.schema_id,
                        signer.clone(),
                    )
                    .await
                    {
                        error!(
                            TARGET_VALIDATION,
                            "Can not create validator {}: {}", signer, e
                        );
                    }
                }
            }
            ValidationMessage::Response {
                validation_res,
                sender,
            } => {
                if !self.reboot {
                    // If node is in validator list
                    if self.check_validator(sender.clone()) {
                        match validation_res {
                            ValidationRes::Signature(signature) => {
                                self.valid_validation = true;
                                self.validators_response.push(
                                    ProtocolsSignatures::Signature(signature),
                                )
                            }
                            ValidationRes::TimeOut(timeout) => self
                                .validators_timeout
                                .push(ProtocolsSignatures::TimeOut(timeout)),
                            ValidationRes::Error(error) => {
                                self.errors = format!(
                                    "{} who: {}, error: {}.",
                                    self.errors, sender, error
                                );
                            }
                            ValidationRes::Reboot => {
                                let governance_id =
                                    self.actual_proof.governance_id.clone();
                                if let Err(e) = send_reboot_to_req(
                                    ctx,
                                    &self.request_id,
                                    governance_id,
                                )
                                .await
                                {
                                    error!(
                                        TARGET_VALIDATION,
                                        "Response, can not send reboot to Request actor: {}",
                                        e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                }
                                self.reboot = true;

                                if let Err(e) = self.end_validators(ctx).await {
                                    error!(
                                        TARGET_VALIDATION,
                                        "Response, can not end validators: {}",
                                        e
                                    );
                                    return Err(emit_fail(ctx, e).await);
                                };

                                return Ok(());
                            }
                        };

                        if self.quorum.check_quorum(
                            self.validators_quantity,
                            self.validators_response.len() as u32,
                        ) && self.valid_validation
                        {
                            if let Err(e) =
                                self.send_validation_to_req(ctx, true).await
                            {
                                error!(
                                    TARGET_VALIDATION,
                                    "Response, can not send validation response to Request actor: {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            };
                        } else if self.validators.is_empty() {
                            // we have received all the responses and the quorum has not been met

                            if let Err(e) =
                                self.send_validation_to_req(ctx, false).await
                            {
                                error!(
                                    TARGET_VALIDATION,
                                    "Response, can not send validation response to Request actor: {}",
                                    e
                                );
                                return Err(emit_fail(ctx, e).await);
                            };
                        }
                    } else {
                        warn!(
                            TARGET_VALIDATION,
                            "Response, A response has been received from someone we were not expecting."
                        );
                    }
                }
            }
        };
        Ok(())
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Validation>,
    ) -> ChildAction {
        error!(TARGET_VALIDATION, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}

#[cfg(test)]
pub mod tests {
    use core::panic;
    use identity::identifier::derive::digest::DigestDerivator;
    use std::time::Duration;
    use test_log::test;

    use actor::{ActorPath, ActorRef, Sink, SystemRef};
    use identity::{
        identifier::DigestIdentifier,
        keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
    };

    use crate::{
        CreateRequest, EOLRequest, EventRequest, Governance, HashId, Node,
        NodeMessage, NodeResponse, Signed, Subject, SubjectMessage,
        SubjectResponse, ValueWrapper,
        helpers::db::ExternalDB,
        model::{Namespace, SignTypesNode, event::LedgerValue},
        query::Query,
        request::{
            RequestHandler, RequestHandlerMessage, RequestHandlerResponse,
        },
        subject::event::{
            LedgerEvent, LedgerEventMessage, LedgerEventResponse,
        },
        system::tests::create_system,
    };

    pub async fn create_subject_gov() -> (
        SystemRef,
        ActorRef<Node>,
        ActorRef<RequestHandler>,
        ActorRef<Query>,
        ActorRef<Subject>,
        ActorRef<LedgerEvent>,
        DigestIdentifier,
    ) {
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let system = create_system().await;

        let node = Node::new(&node_keys).unwrap();
        let node_actor = system.create_root_actor("node", node).await.unwrap();

        let request = RequestHandler::new(node_keys.key_identifier());
        let request_actor =
            system.create_root_actor("request", request).await.unwrap();

        let query_actor = system
            .create_root_actor("query", Query::new(node_keys.key_identifier()))
            .await
            .unwrap();

        let ext_db: ExternalDB = system.get_helper("ext_db").await.unwrap();

        let sink =
            Sink::new(request_actor.subscribe(), ext_db.get_request_handler());
        system.run_sink(sink).await;

        let create_req = EventRequest::Create(CreateRequest {
            name: Some("Name".to_string()),
            description: Some("Description".to_string()),
            governance_id: DigestIdentifier::default(),
            schema_id: "governance".to_owned(),
            namespace: Namespace::new(),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                create_req.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: create_req,
            signature,
        };

        let RequestHandlerResponse::Ok(response) = request_actor
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req.clone(),
            })
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        let owned_subj = response.subject_id;

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let subject_actor: ActorRef<Subject> = system
            .get_actor(&ActorPath::from(format!("/user/node/{}", owned_subj)))
            .await
            .unwrap();

        let ledger_event_actor: ActorRef<LedgerEvent> = system
            .get_actor(&ActorPath::from(format!(
                "/user/node/{}/ledger_event",
                owned_subj
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

        let SubjectResponse::Metadata(metadata) = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap()
        else {
            panic!("Invalid response")
        };

        assert_eq!(last_event.content.subject_id.to_string(), owned_subj);
        assert_eq!(last_event.content.event_request, signed_event_req);
        assert_eq!(last_event.content.sn, 0);
        assert_eq!(last_event.content.gov_version, 0);
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

        assert_eq!(metadata.subject_id.to_string(), owned_subj);
        assert_eq!(metadata.name.unwrap(), "Name");
        assert_eq!(metadata.description.unwrap(), "Description");
        assert_eq!(metadata.governance_id.to_string(), "");
        assert_eq!(metadata.genesis_gov_version, 0);
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 0);
        assert_eq!(metadata.owner, node_keys.key_identifier());
        assert!(metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 0);
        // TODO MEJORAR
        assert!(!gov.members.is_empty());
        assert!(gov.roles_schema.is_empty());
        assert!(gov.schemas.is_empty());
        assert!(gov.policies_schema.is_empty());

        (
            system,
            node_actor,
            request_actor,
            query_actor,
            subject_actor,
            ledger_event_actor,
            metadata.subject_id,
        )
    }

    #[test(tokio::test)]
    async fn test_create_req() {
        let _ = create_subject_gov().await;
    }

    #[test(tokio::test)]
    async fn test_eol_req() {
        let (
            _system,
            node_actor,
            request_actor,
            _query_actor,
            subject_actor,
            ledger_event_actor,
            subject_id,
        ) = create_subject_gov().await;

        let eol_reques = EventRequest::EOL(EOLRequest {
            subject_id: subject_id.clone(),
        });

        let response = node_actor
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                eol_reques.clone(),
            )))
            .await
            .unwrap();
        let NodeResponse::SignRequest(signature) = response else {
            panic!("Invalid Response")
        };

        let signed_event_req = Signed {
            content: eol_reques,
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

        tokio::time::sleep(Duration::from_secs(3)).await;

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
            LedgerValue::Patch(ValueWrapper(serde_json::Value::String(
                "[]".to_owned(),
            ),))
        );
        assert!(last_event.content.eval_success.is_none());
        assert!(!last_event.content.appr_required);
        assert!(last_event.content.appr_success.is_none());
        assert!(last_event.content.vali_success);
        assert!(last_event.content.evaluators.is_none());
        assert!(last_event.content.approvers.is_none(),);
        assert!(!last_event.content.validators.is_empty());

        assert_eq!(metadata.subject_id, subject_id);
        assert_eq!(metadata.governance_id.to_string(), "");
        assert_eq!(metadata.name.unwrap(), "Name");
        assert_eq!(metadata.description.unwrap(), "Description");
        assert_eq!(metadata.genesis_gov_version, 0);
        assert_eq!(metadata.schema_id, "governance");
        assert_eq!(metadata.namespace, Namespace::new());
        assert_eq!(metadata.sn, 1);
        assert!(!metadata.active);

        let gov = Governance::try_from(metadata.properties).unwrap();
        assert_eq!(gov.version, 1);
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
}
