// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use rush::{
    Actor, ActorContext, ActorError, ActorPath, ActorRef, ChildAction, Handler,
    Message,
};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::{
    Signed,
    distribution::{Distribution, DistributionMessage, DistributionType},
    model::{
        common::{emit_fail, subject_owner},
        event::{Event as KoreEvent, Ledger, ProtocolsSignatures},
    },
    subject::{Subject, SubjectMessage, SubjectResponse},
    validation::proof::ValidationProof,
};

const TARGET_MANUAL_DISTRIBUTION: &str = "Kore-Node-ManualDistribution";

pub struct ManualDistribution {
    our_key: KeyIdentifier,
}

impl ManualDistribution {
    pub fn new(our_key: KeyIdentifier) -> Self {
        Self { our_key }
    }
    async fn get_last_ledger(
        ctx: &mut ActorContext<ManualDistribution>,
        subject_id: &str,
    ) -> Result<
        (
            Vec<Signed<Ledger>>,
            Box<Option<Signed<KoreEvent>>>,
            Box<Option<ValidationProof>>,
            Option<Vec<ProtocolsSignatures>>,
        ),
        ActorError,
    > {
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));
        let subject_actor: Option<ActorRef<Subject>> =
            ctx.system().get_actor(&subject_path).await;

        let response = if let Some(subject_actor) = subject_actor {
            subject_actor.ask(SubjectMessage::GetLastLedger).await?
        } else {
            return Err(ActorError::NotFound(subject_path));
        };

        match response {
            SubjectResponse::Ledger {
                ledger,
                last_event,
                last_proof,
                prev_event_validation_response,
            } => Ok((
                ledger,
                last_event,
                last_proof,
                prev_event_validation_response,
            )),
            _ => Err(ActorError::UnexpectedResponse(
                subject_path,
                "SubjectResponse::Ledger".to_owned(),
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ManualDistributionMessage {
    Update(DigestIdentifier),
}

impl Message for ManualDistributionMessage {}

#[async_trait]
impl Actor for ManualDistribution {
    type Message = ManualDistributionMessage;
    type Event = ();
    type Response = ();

    async fn pre_start(
        &mut self,
        _ctx: &mut rush::ActorContext<Self>,
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
impl Handler<ManualDistribution> for ManualDistribution {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: ManualDistributionMessage,
        ctx: &mut rush::ActorContext<ManualDistribution>,
    ) -> Result<(), ActorError> {
        match msg {
            ManualDistributionMessage::Update(subject_id) => {
                let (is_owner, _is_pending) =
                    subject_owner(ctx, &subject_id.to_string()).await?;

                if !is_owner {
                    let e = "We are not subject owner";
                    warn!(TARGET_MANUAL_DISTRIBUTION, "Update, {}", e);
                    return Err(ActorError::Functional(e.to_owned()));
                }

                let distribution = Distribution::new(
                    self.our_key.clone(),
                    DistributionType::Manual,
                );
                let request_id = format!("M_{}", subject_id);

                let (
                    ledger,
                    last_event,
                    last_proof,
                    prev_event_validation_response,
                ) = Self::get_last_ledger(ctx, &subject_id.to_string()).await?;

                let Some(last_event) = *last_event else {
                    let e = "Can not obtain last signed event";
                    error!(TARGET_MANUAL_DISTRIBUTION, "Update, {}", e);
                    return Err(ActorError::Functional(e.to_string()));
                };

                let Some(last_proof) = *last_proof else {
                    let e = "Can not obtain last proof";
                    error!(TARGET_MANUAL_DISTRIBUTION, "Update, {}", e);
                    return Err(ActorError::Functional(e.to_string()));
                };

                let Some(prev_event_validation_response) =
                    prev_event_validation_response
                else {
                    let e =
                        "Can not obtain previous event validation responses";
                    error!(TARGET_MANUAL_DISTRIBUTION, "Update, {}", e);
                    return Err(ActorError::Functional(e.to_string()));
                };

                let ledger = if ledger.len() != 1 {
                    let e = "Failed to get the latest event from the ledger";
                    error!(TARGET_MANUAL_DISTRIBUTION, "Update, {}", e);
                    return Err(ActorError::Functional(e.to_string()));
                } else {
                    ledger[0].clone()
                };

                let distribution_actor = ctx.create_child(&request_id, distribution).await.map_err(|e| {
                    warn!(TARGET_MANUAL_DISTRIBUTION, "Update, Can not create distribution child: {}", e);
                    ActorError::Functional("There was already a manual distribution in progress".to_owned())
                })?;

                if let Err(e) = distribution_actor
                    .tell(DistributionMessage::Create {
                        request_id,
                        event: Box::new(last_event),
                        ledger: Box::new(ledger),
                        last_proof: Box::new(last_proof),
                        prev_event_validation_response,
                    })
                    .await
                {
                    let e = format!("Can not create manual update: {}", e);
                    error!(TARGET_MANUAL_DISTRIBUTION, "Update, {}", e);
                    return Err(ActorError::Functional(e.to_string()));
                };

                Ok(())
            }
        }
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<ManualDistribution>,
    ) -> ChildAction {
        error!(TARGET_MANUAL_DISTRIBUTION, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}
