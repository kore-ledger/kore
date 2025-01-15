// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, ChildAction, Error as ActorError,
    Handler, Message,
};
use async_trait::async_trait;
use distributor::{Distributor, DistributorMessage};
use identity::identifier::KeyIdentifier;
use tracing::error;

use crate::{
    governance::model::Roles,
    model::{
        common::{emit_fail, get_gov},
        event::{Ledger, ProtocolsSignatures},
    },
    request::manager::{RequestManager, RequestManagerMessage},
    validation::proof::ValidationProof,
    Event as KoreEvent, Signed,
};

pub mod distributor;

const TARGET_DISTRIBUTION: &str = "Kore-Distribution";

#[derive(Default)]
pub struct Distribution {
    witnesses: HashSet<KeyIdentifier>,
    node_key: KeyIdentifier,
    request_id: String,
}

impl Distribution {
    pub fn new(node_key: KeyIdentifier) -> Self {
        Distribution {
            node_key,
            ..Default::default()
        }
    }

    fn check_witness(&mut self, witness: KeyIdentifier) -> bool {
        self.witnesses.remove(&witness)
    }

    async fn create_distributors(
        &self,
        ctx: &mut ActorContext<Distribution>,
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
        signer: KeyIdentifier,
        last_proof: ValidationProof,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    ) -> Result<(), ActorError> {
        let child = ctx
            .create_child(
                &format!("{}", signer),
                Distributor {
                    node: signer.clone(),
                },
            )
            .await;
        let distributor_actor = match child {
            Ok(child) => child,
            Err(e) => return Err(e),
        };

        let our_key = self.node_key.clone();

        distributor_actor
            .tell(DistributorMessage::NetworkDistribution {
                ledger,
                event,
                node_key: signer,
                our_key,
                last_proof,
                prev_event_validation_response,
            })
            .await
    }

    async fn end_request(
        &self,
        ctx: &mut ActorContext<Distribution>,
    ) -> Result<(), ActorError> {
        let req_path =
            ActorPath::from(format!("/user/request/{}", self.request_id));
        let req_actor: Option<ActorRef<RequestManager>> =
            ctx.system().get_actor(&req_path).await;

        if let Some(req_actor) = req_actor {
            req_actor.tell(RequestManagerMessage::FinishRequest).await?;
        } else {
            return Err(ActorError::NotFound(req_path));
        };

        Ok(())
    }
}

#[async_trait]
impl Actor for Distribution {
    type Event = ();
    type Message = DistributionMessage;
    type Response = ();
}

#[derive(Debug, Clone)]
pub enum DistributionMessage {
    Create {
        request_id: String,
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
        last_proof: Box<ValidationProof>,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
    Response {
        sender: KeyIdentifier,
    },
}

impl Message for DistributionMessage {}

#[async_trait]
impl Handler<Distribution> for Distribution {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: DistributionMessage,
        ctx: &mut ActorContext<Distribution>,
    ) -> Result<(), ActorError> {
        match msg {
            DistributionMessage::Create {
                request_id,
                event,
                ledger,
                last_proof,
                prev_event_validation_response,
            } => {
                self.request_id = request_id;
                let subject_id = ledger.content.subject_id.clone();
                // TODO, a lo mejor en el comando de creación se pueden incluir el namespace y el schema
                let governance =
                    match get_gov(ctx, &subject_id.to_string()).await {
                        Ok(gov) => gov,
                        Err(e) => {
                            error!(
                                TARGET_DISTRIBUTION,
                                "Create, can not get governance: {}", e
                            );
                            return Err(emit_fail(ctx, e).await);
                        }
                    };

                let mut witnesses = if last_proof.schema_id == "governance" {
                    governance.members_to_key_identifier()
                } else {
                    governance
                        .get_signers(
                            Roles::WITNESS,
                            &last_proof.schema_id,
                            last_proof.namespace.clone(),
                        )
                        .0
                };

                let _ = witnesses.remove(&self.node_key);

                if witnesses.is_empty() {
                    if let Err(e) = self.end_request(ctx).await {
                        error!(
                            TARGET_DISTRIBUTION,
                            "Create, can not end distribution: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    };
                    return Ok(());
                }

                self.witnesses.clone_from(&witnesses);

                let last_proof = *last_proof;
                for witness in witnesses {
                    self.create_distributors(
                        ctx,
                        event.clone(),
                        ledger.clone(),
                        witness,
                        last_proof.clone(),
                        prev_event_validation_response.clone(),
                    )
                    .await?
                }
            }
            DistributionMessage::Response { sender } => {
                if self.check_witness(sender) && self.witnesses.is_empty() {
                    if let Err(e) = self.end_request(ctx).await {
                        error!(
                            TARGET_DISTRIBUTION,
                            "Response, can not end distribution: {}", e
                        );
                        return Err(emit_fail(ctx, e).await);
                    };
                }
            }
        }

        Ok(())
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Distribution>,
    ) -> ChildAction {
        error!(TARGET_DISTRIBUTION, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}
