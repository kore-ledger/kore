// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    model::{
        common::emit_fail,
        event::ProtocolsSignatures,
    }, validation::proof::ValidationProof
};
use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event,
    Handler, Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use store::store::PersistentActor;
use tracing::{error, warn};

use crate::db::Storable;

const TARGET_VALIDATA: &str = "Kore-Subject-ValiData";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValiData {
    previous_proof: Option<ValidationProof>,
    prev_event_validation_response: Vec<ProtocolsSignatures>,
}

#[derive(Debug, Clone)]
pub enum ValiDataMessage {
    UpdateValiData { 
        previous_proof: ValidationProof,
        prev_event_validation_response: Vec<ProtocolsSignatures>
    },
    GetLastValiData,
}

impl Message for ValiDataMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValiDataEvent {    
    pub previous_proof: ValidationProof,
    pub prev_event_validation_response: Vec<ProtocolsSignatures>
}

impl Event for ValiDataEvent {}

#[derive(Debug, Clone)]
pub enum ValiDataResponse {
    LastValiData {
        previous_proof: Option<ValidationProof>,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
    Ok,
}

impl Response for ValiDataResponse {}

#[async_trait]
impl Actor for ValiData {
    type Event = ValiDataEvent;
    type Message = ValiDataMessage;
    type Response = ValiDataResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        let prefix = ctx.path().parent().key();
        self.init_store("vali_data", Some(prefix), true, ctx).await
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<ValiData> for ValiData {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: ValiDataMessage,
        ctx: &mut ActorContext<ValiData>,
    ) -> Result<ValiDataResponse, ActorError> {
        match msg {
            ValiDataMessage::UpdateValiData { previous_proof, prev_event_validation_response } => {
                self.on_event(
                    ValiDataEvent { previous_proof, prev_event_validation_response },
                    ctx,
                ).await;

                Ok(ValiDataResponse::Ok)
            },
            ValiDataMessage::GetLastValiData =>
                Ok(ValiDataResponse::LastValiData { previous_proof: self.previous_proof.clone(), prev_event_validation_response: self.prev_event_validation_response.clone() }),
        }
    }

    async fn on_event(
        &mut self,
        event: ValiDataEvent,
        ctx: &mut ActorContext<ValiData>,
    ) {
        if let Err(e) = self.persist_light(&event, ctx).await {
            error!(TARGET_VALIDATA, "OnEvent, can not persist information: {}", e);
            emit_fail(ctx, e).await;
        };

        if let Err(e) = ctx.publish_event(event).await {
            error!(TARGET_VALIDATA, "PublishEvent, can not publish event: {}", e);
            emit_fail(ctx, e).await;
        }
    }
}

#[async_trait]
impl PersistentActor for ValiData {
    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        self.previous_proof = Some(event.previous_proof.clone());
        self.prev_event_validation_response = event.prev_event_validation_response.clone();

        Ok(())
    }
}

impl Storable for ValiData {}
