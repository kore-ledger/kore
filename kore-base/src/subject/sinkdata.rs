// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::model::common::emit_fail;

use super::Metadata;

const TARGET_SINKDATA: &str = "Kore-Subject-Sinkdata";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SinkData;

#[derive(Debug, Clone)]
pub enum SinkDataMessage {
    SafeMetadata(Metadata),
}

impl Message for SinkDataMessage {}

#[derive(Debug, Clone)]
pub enum SinkDataResponse {
    None,
}

impl Response for SinkDataResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkDataEvent {
    pub metadata: Metadata,
}

impl Event for SinkDataEvent {}

#[async_trait]
impl Actor for SinkData {
    type Event = SinkDataEvent;
    type Message = SinkDataMessage;
    type Response = SinkDataResponse;

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
impl Handler<SinkData> for SinkData {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: SinkDataMessage,
        ctx: &mut actor::ActorContext<SinkData>,
    ) -> Result<SinkDataResponse, ActorError> {
        match msg {
            SinkDataMessage::SafeMetadata(metadata) => {
                self.on_event(SinkDataEvent { metadata }, ctx).await;
            }
        };

        Ok(SinkDataResponse::None)
    }

    async fn on_event(
        &mut self,
        event: SinkDataEvent,
        ctx: &mut ActorContext<SinkData>,
    ) {
        if let Err(e) = ctx.publish_event(event).await {
            error!(TARGET_SINKDATA, "OnEvent, can not publish event: {}", e);
            emit_fail(ctx, e).await;
        };
    }
}
