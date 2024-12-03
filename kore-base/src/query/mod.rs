// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Handler, Message,
    Response, SystemEvent,
};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

use crate::helpers::db::{
    EventDB, ExternalDB, Paginator, Querys, SignaturesDB, SubjectDB,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query {
    key: KeyIdentifier,
}

impl Query {
    pub fn new(key: KeyIdentifier) -> Self {
        Self { key }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryMessage {
    GetSubject {
        subject_id: String,
    },
    GetSignatures {
        subject_id: String,
    },
    GetEvents {
        subject_id: String,
        quantity: Option<u64>,
        page: Option<u64>,
    },
    GetRequestState {
        request_id: String,
    },
    GetApproval {
        subject_id: String,
    },
}

impl Message for QueryMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryResponse {
    Signatures {
        signatures: SignaturesDB,
    },
    Subject {
        subject: SubjectDB,
    },
    Events {
        events: Vec<EventDB>,
        paginator: Paginator,
    },
    RequestState(String),
    ApprovalState {
        request: String,
        state: String,
    },
}

impl Response for QueryResponse {}

#[async_trait]
impl Actor for Query {
    type Message = QueryMessage;
    type Event = ();
    type Response = QueryResponse;

    async fn pre_start(
        &mut self,
        _ctx: &mut actor::ActorContext<Self>,
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
impl Handler<Query> for Query {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: QueryMessage,
        ctx: &mut actor::ActorContext<Query>,
    ) -> Result<QueryResponse, ActorError> {
        let Some(helper): Option<ExternalDB> =
            ctx.system().get_helper("ext_db").await
        else {
            ctx.system().send_event(SystemEvent::StopSystem).await;
            return Err(ActorError::NotHelper("ext_db".to_owned()));
        };

        match msg {
            QueryMessage::GetSignatures { subject_id } => {
                let signatures = helper
                    .get_signatures(&subject_id)
                    .await
                    .map_err(|e| ActorError::Functional(e.to_string()))?;
                Ok(QueryResponse::Signatures { signatures })
            }
            QueryMessage::GetSubject { subject_id } => {
                let subject = helper
                    .get_subject_state(&subject_id)
                    .await
                    .map_err(|e| ActorError::Functional(e.to_string()))?;
                Ok(QueryResponse::Subject { subject })
            }
            QueryMessage::GetEvents {
                subject_id,
                quantity,
                page,
            } => {
                let (events, paginator) = helper
                    .get_events(&subject_id, quantity, page)
                    .await
                    .map_err(|e| ActorError::Functional(e.to_string()))?;
                Ok(QueryResponse::Events { events, paginator })
            }
            QueryMessage::GetRequestState { request_id } => {
                let res = helper
                    .get_request_id_status(&request_id)
                    .await
                    .map_err(|e| ActorError::Functional(e.to_string()))?;
                Ok(QueryResponse::RequestState(res))
            }
            QueryMessage::GetApproval { subject_id } => {
                let (request, state) = helper
                    .get_approve_req(&subject_id)
                    .await
                    .map_err(|e| ActorError::Functional(e.to_string()))?;
                Ok(QueryResponse::ApprovalState { request, state })
            }
        }
    }
}
