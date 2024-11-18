// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler,
    Message, Response,
};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

use crate::{
    approval::approver::{
        ApprovalState, ApprovalStateRes, Approver, ApproverMessage,
    },
    helpers::db::{ExternalDB, Querys},
    request::state,
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
    GetRequestState { request_id: String },
    GetApproval { subject_id: String },
}

impl Message for QueryMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryResponse {
    RequestState(String),
    ApprovalState { request: String, state: String },
    Response(String),
    Error(String),
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
            todo!()
        };

        // Sacar el estado de una request
        // Sacar la aprobación

        // Obtener los nodos autorizados y los testigos.

        // Obtener Todas las governanzas
        // Obtener todos los sujetos de una determinada governanza
        // Obtener todos los schemas de una determinada governanza

        // Obtener el estado de un sujeto.
        // Obtener sus eventos.

        match msg {
            QueryMessage::GetRequestState { request_id } => {
                match helper.get_request_id_status(&request_id).await {
                    Ok(res) => Ok(QueryResponse::RequestState(res)),
                    Err(e) => Ok(QueryResponse::Error(e.to_string())),
                }
            }
            QueryMessage::GetApproval { subject_id } => {
                match helper.get_approve_req(&subject_id).await {
                    Ok((request, state)) => {
                        Ok(QueryResponse::ApprovalState { request, state })
                    }
                    Err(e) => Ok(QueryResponse::Error(e.to_string())),
                }
            }
        }
    }
}
