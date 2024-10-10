// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Event, Handler, Message, Response
};
use async_trait::async_trait;
use identity::{
    identifier::{
        derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
    },
    keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::error;
use std::collections::{HashMap, HashSet, VecDeque};
use store::store::PersistentActor;

use crate::{
    db::Storable, governance::model::Roles, init_state, model::{
        common::{get_gov, get_metadata, get_sign}, event::{LedgerValue, ProofEvent}, signature, SignTypesNode
    }, node, subject::{self, CreateSubjectData, SubjectID}, validation::{proof::EventProof, ValidationEvent}, CreateRequest, Error, Event as KoreEvent, EventRequest, FactRequest, HashId, Node, NodeMessage, NodeResponse, Signed, Validation, ValidationInfo, ValidationMessage, ValueWrapper, DIGEST_DERIVATOR
};

use super::state::RequestSate;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestManager {
    id: DigestIdentifier,
    state: RequestSate,
    subject_id: DigestIdentifier,
    request: Signed<EventRequest>,
}

impl RequestManager {
    async fn send_validation(&self, ctx: &mut ActorContext<RequestManager>, val_info: ValidationInfo) -> Result<(), Error> {
        let validation_path = ActorPath::from(format!("/user/node/{}/validation", self.subject_id));
        let validation_actor: Option<ActorRef<Validation>> = ctx.system().get_actor(&validation_path).await;

        if let Some(validation_actor) = validation_actor {
            if let Err(e) = validation_actor.tell(ValidationMessage::Create { request_id: self.id.clone(), info: val_info }).await {
                todo!()
            }
        } else {
            todo!()
        }

        Ok(())
    }

    async fn validation(&self, ctx: &mut ActorContext<RequestManager>) -> Result<ValidationInfo, Error> {
        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!("Error getting derivator");
            DigestDerivator::Blake3_256
        };

        let gov = match get_gov(ctx, self.subject_id.clone()).await {
            Ok(gov) => gov,
            Err(e) => todo!(),
        };

        let metadata =
            match get_metadata(ctx, self.subject_id.clone()).await {
                Ok(metadata) => metadata,
                Err(e) => todo!(),
            };

        let state_hash = match self.request.hash_id(derivator) {
            Ok(hash) => hash,
            Err(e) => todo!()
        };

        let event_proof = EventProof::from(self.request.content.clone());
            
        let event_proof = ProofEvent {
            subject_id: metadata.subject_id.clone(),
            event_proof,
            sn: metadata.sn.clone(),
            gov_version: gov.version,
            value: LedgerValue::Patch(ValueWrapper(json!([]))),
            state_hash,
            eval_success: None,
            appr_required: false,
            appr_success: None,
            hash_prev_event: metadata.last_event_hash.clone(),
            evaluators: None,
            approvers: None,
        };

        let signature = match get_sign(ctx, SignTypesNode::ValidationProofEvent(event_proof.clone())).await {
            Ok(signature) => signature,
            Err(e) => todo!()
        };

        let event_proof = Signed { content: event_proof, signature };

        let val_info = ValidationInfo {
            metadata,
            event_proof,
        };

        self.send_validation(ctx, val_info.clone()).await?;
        Ok(val_info)
    }
}

#[derive(Debug, Clone)]
pub enum RequestManagerMessage {
    Fact,
    Other
}

impl Message for RequestManagerMessage {}

#[derive(Debug, Clone)]
pub enum RequestManagerResponse {
    Ok(String),
    Error(Error),
    None,
}

impl Response for RequestManagerResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestManagerEvent {
    ChangeState {
        state: RequestSate
    }
}

impl Event for RequestManagerEvent {}

#[async_trait]
impl Actor for RequestManager {
    type Event = RequestManagerEvent;
    type Message = RequestManagerMessage;
    type Response = RequestManagerResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("request_handler", None, false, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl PersistentActor for RequestManager {
    /// Change node state.
    fn apply(&mut self, event: &Self::Event) {
        match event {
            RequestManagerEvent::ChangeState { state } => {
                self.state = state.clone();
            },
        }
    }
}

#[async_trait]
impl Handler<RequestManager> for RequestManager {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: RequestManagerMessage,
        ctx: &mut actor::ActorContext<RequestManager>,
    ) -> Result<RequestManagerResponse, ActorError> {
        match msg {
            RequestManagerMessage::Fact => todo!(),
            RequestManagerMessage::Other => {
                let val_info = match self.validation(ctx).await {
                    Ok(val_info) => val_info,
                    Err(e) => todo!()
                };

                self.on_event(RequestManagerEvent::ChangeState { state: RequestSate::Validation(val_info) }, ctx).await;
            }
        }

        Ok(RequestManagerResponse::None)
    }

    async fn on_event(
        &mut self,
        event: RequestManagerEvent,
        ctx: &mut ActorContext<RequestManager>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            // TODO Propagar error.
        };
    }
}

#[async_trait]
impl Storable for RequestManager {}
