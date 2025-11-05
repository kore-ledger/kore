use std::fmt::Display;

use async_trait::async_trait;
use rush::{
    Actor, ActorContext, ActorError, ActorPath, Event, Handler, Message,
    Response,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::error;

use crate::{model::common::emit_fail, subject::Metadata};
const TARGET_SINKDATA: &str = "Kore-Subject-Sinkdata";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SinkData {
    pub controller_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SinkDataMessage {
    UpdateState(Box<Metadata>),
    Create {
        governance_id: Option<String>,
        subject_id: String,
        owner: String,
        schema_id: String,
        namespace: String,
        sn: u64,
    },
    Fact {
        governance_id: Option<String>,
        subject_id: String,
        schema_id: String,
        issuer: String,
        owner: String,
        payload: Value,
        sn: u64,
    },
    Transfer {
        governance_id: Option<String>,
        subject_id: String,
        schema_id: String,
        owner: String,
        new_owner: String,
        sn: u64,
    },
    Confirm {
        governance_id: Option<String>,
        subject_id: String,
        schema_id: String,
        sn: u64,
    },
    Reject {
        governance_id: Option<String>,
        subject_id: String,
        schema_id: String,
        sn: u64,
    },
    EOL {
        governance_id: Option<String>,
        subject_id: String,
        schema_id: String,
        sn: u64,
    },
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Eq, Ord, PartialEq, PartialOrd,
)]
pub enum SinkTypes {
    Create,
    Fact,
    Transfer,
    Confirm,
    Reject,
    EOL,
    All,
}

impl Display for SinkTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinkTypes::Create => write!(f, "Create"),
            SinkTypes::Fact => write!(f, "Fact"),
            SinkTypes::Transfer => write!(f, "Transfer"),
            SinkTypes::Confirm => write!(f, "Confirm"),
            SinkTypes::Reject => write!(f, "Reject"),
            SinkTypes::EOL => write!(f, "EOL"),
            SinkTypes::All => write!(f, "All"),
        }
    }
}

impl TryFrom<SinkDataMessage> for SinkTypes {
    type Error = &'static str;

    fn try_from(value: SinkDataMessage) -> Result<Self, Self::Error> {
        match value {
            SinkDataMessage::UpdateState(..) => {
                Err("UpdateState cannot be converted to SinkTypes")
            }
            SinkDataMessage::Create { .. } => Ok(SinkTypes::Create),
            SinkDataMessage::Fact { .. } => Ok(SinkTypes::Fact),
            SinkDataMessage::Transfer { .. } => Ok(SinkTypes::Transfer),
            SinkDataMessage::Confirm { .. } => Ok(SinkTypes::Confirm),
            SinkDataMessage::Reject { .. } => Ok(SinkTypes::Reject),
            SinkDataMessage::EOL { .. } => Ok(SinkTypes::EOL),
        }
    }
}

impl From<String> for SinkTypes {
    fn from(value: String) -> Self {
        match value.trim() {
            "Create" => Self::Create,
            "Fact" => Self::Fact,
            "Transfer" => Self::Transfer,
            "Confirm" => Self::Confirm,
            "Reject" => Self::Reject,
            "EOL" => Self::EOL,
            _ => Self::All,
        }
    }
}

impl SinkDataMessage {
    pub fn get_subject_schema(&self) -> (String, String) {
        match self {
            SinkDataMessage::UpdateState(metadata) => {
                (metadata.subject_id.to_string(), metadata.schema_id.clone())
            }
            SinkDataMessage::Create {
                subject_id,
                schema_id,
                ..
            }
            | SinkDataMessage::Fact {
                subject_id,
                schema_id,
                ..
            }
            | SinkDataMessage::Transfer {
                subject_id,
                schema_id,
                ..
            }
            | SinkDataMessage::Confirm {
                subject_id,
                schema_id,
                ..
            }
            | SinkDataMessage::Reject {
                subject_id,
                schema_id,
                ..
            }
            | SinkDataMessage::EOL {
                subject_id,
                schema_id,
                ..
            } => (subject_id.clone(), schema_id.clone()),
        }
    }
}

impl Message for SinkDataMessage {}

#[derive(Debug, Clone)]
pub enum SinkDataResponse {
    None,
}

impl Response for SinkDataResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkDataEvent {
    pub event: SinkDataMessage,
    pub controller_id: String,
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
        ctx: &mut rush::ActorContext<SinkData>,
    ) -> Result<SinkDataResponse, ActorError> {
        self.on_event(
            SinkDataEvent {
                event: msg,
                controller_id: self.controller_id.clone(),
            },
            ctx,
        )
        .await;

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
