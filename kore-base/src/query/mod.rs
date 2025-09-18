// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use rush::{
    Actor, ActorContext, ActorPath, ActorError, Handler, Message,
    Response,
};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::helpers::db::{
    ExternalDB, Querys,
    common::{
        ApproveInfo, EventInfo, PaginatorEvents, RequestInfo, SignaturesInfo,
        SubjectInfo,
    },
};

const TARGET_QUERY: &str = "Kore-Query";

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
        reverese: Option<bool>,
    },
    GetEventSn {
        subject_id: String,
        sn: u64,
    },
    GetFirstOrEndEvents {
        subject_id: String,
        quantity: Option<u64>,
        reverse: Option<bool>,
        success: Option<bool>,
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
    Signatures(SignaturesInfo),
    Subject(SubjectInfo),
    PaginatorEvents(PaginatorEvents),
    Event(EventInfo),
    Events(Vec<EventInfo>),
    RequestState(RequestInfo),
    ApprovalState(ApproveInfo),
}

impl Response for QueryResponse {}

#[async_trait]
impl Actor for Query {
    type Message = QueryMessage;
    type Event = ();
    type Response = QueryResponse;

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
impl Handler<Query> for Query {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: QueryMessage,
        ctx: &mut rush::ActorContext<Query>,
    ) -> Result<QueryResponse, ActorError> {
        let Some(helper): Option<ExternalDB> =
            ctx.system().get_helper("ext_db").await
        else {
            error!(TARGET_QUERY, "Can not obtain ext_db helper");
            ctx.system().stop_system();
            return Err(ActorError::NotHelper("ext_db".to_owned()));
        };

        match msg {
            QueryMessage::GetSignatures { subject_id } => {
                let signatures =
                    helper.get_signatures(&subject_id).await.map_err(|e| {
                        warn!(
                            TARGET_QUERY,
                            "GetSignatures, Can not obtain signatures: {}", e
                        );
                        ActorError::Functional(e.to_string())
                    })?;
                Ok(QueryResponse::Signatures(signatures))
            }
            QueryMessage::GetSubject { subject_id } => {
                let subject = helper
                    .get_subject_state(&subject_id)
                    .await
                    .map_err(|e| {
                        warn!(
                            TARGET_QUERY,
                            "GetSubject, Can not obtain subject state: {}", e
                        );
                        ActorError::Functional(e.to_string())
                    })?;
                Ok(QueryResponse::Subject(subject))
            }
            QueryMessage::GetEvents {
                subject_id,
                quantity,
                page,
                reverese,
            } => {
                let data = helper
                    .get_events(&subject_id, quantity, page, reverese)
                    .await
                    .map_err(|e| {
                        warn!(
                            TARGET_QUERY,
                            "GetEvents, Can not obtain events: {}", e
                        );
                        ActorError::Functional(e.to_string())
                    })?;
                Ok(QueryResponse::PaginatorEvents(data))
            }
            QueryMessage::GetEventSn { subject_id, sn } => {
                let data = helper
                    .get_events_sn(&subject_id, sn)
                    .await
                    .map_err(|e| {
                        warn!(
                            TARGET_QUERY,
                            "GetEventSn, Can not obtain event sn: {}", e
                        );
                        ActorError::Functional(e.to_string())
                    })?;
                Ok(QueryResponse::Event(data))
            }
            QueryMessage::GetFirstOrEndEvents {
                subject_id,
                quantity,
                reverse,
                success,
            } => {
                let data = helper
                    .get_first_or_end_events(&subject_id, quantity, reverse, success)
                    .await
                    .map_err(|e| {
                        warn!(TARGET_QUERY, "GetFirstOrEndEvents, Can not obtain first or end events: {}", e);
                        ActorError::Functional(e.to_string())})?;
                Ok(QueryResponse::Events(data))
            }
            QueryMessage::GetRequestState { request_id } => {
                let res = helper
                    .get_request_id_status(&request_id)
                    .await
                    .map_err(|e| {
                        warn!(TARGET_QUERY, "GetRequestState, Can not obtain request status: {}", e);
                        ActorError::Functional(e.to_string())})?;
                Ok(QueryResponse::RequestState(res))
            }
            QueryMessage::GetApproval { subject_id } => {
                let data =
                    helper.get_approve_req(&subject_id).await.map_err(|e| {
                        warn!(
                            TARGET_QUERY,
                            "GetApproval, Can not obtain approve request: {}",
                            e
                        );
                        ActorError::Functional(e.to_string())
                    })?;
                Ok(QueryResponse::ApprovalState(data))
            }
        }
    }
}
