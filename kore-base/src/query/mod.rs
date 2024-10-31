// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{Actor, ActorContext, ActorPath, Error as ActorError, Handler, Message, Response};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

use crate::helpers::db::{LocalDB, Querys};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryMessage {
    GetRequestState {
        request_id: String
    },
}

impl Message for QueryMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryResponse {
    RequestState(String),
    Error(String)
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

        let Some(helper): Option<LocalDB> = ctx.system().get_helper("local_db").await else {
            todo!()
        };

        match msg {
            QueryMessage::GetRequestState { request_id} => {
                Ok(QueryResponse::RequestState(helper.get_request_id_status(&request_id).await))
            },
        }
    }
}