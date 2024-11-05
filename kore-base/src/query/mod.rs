// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler, Message, Response};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

use crate::{approval::approver::{ApprovalState, ApprovalStateRes, Approver, ApproverMessage}, helpers::db::{LocalDB, Querys}, request::state};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryMessage {
    GetRequestState {
        request_id: String
    },
    ChangeApprovalState {
        subject_id: String,
        state: ApprovalStateRes
    },
    GetApproval {
        request_id: String
    }

}

impl Message for QueryMessage {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryResponse {
    RequestState(String),
    ApprovalState {
        request: String,
        state: String
    },
    Response(String),
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
                match helper.get_request_id_status(&request_id).await {
                    Ok(res) => Ok(QueryResponse::RequestState(res)),
                    Err(e) => Ok(QueryResponse::Error(e.to_string()))
                }
            },
            QueryMessage::ChangeApprovalState { subject_id, state } => {
                match state.to_string().as_str() {
                  "RespondedAccepted" | "RespondedRejected" => {},
                  _ => return Ok(QueryResponse::Error("Invalid Response".to_owned()))
                };

                let approver_actor: Option<ActorRef<Approver>> = ctx.system().get_actor(&ActorPath::from(format!("/user/node/{}/approver", subject_id))).await;

                if let Some(approver_actor) = approver_actor {
                    if let Err(e) = approver_actor.tell(ApproverMessage::ChangeResponse { response: state.clone() }).await {
                        todo!()
                    }
                } else {
                    todo!()
                };

                Ok(QueryResponse::Response(format!("The approval request for subject {} has changed to {}", subject_id, state.to_string())))
            },
            QueryMessage::GetApproval { request_id } => {
                match helper.get_approve_req(&request_id).await {
                    Ok((request, state)) => Ok(QueryResponse::ApprovalState { request, state }),
                    Err(e) => Ok(QueryResponse::Error(e.to_string()))
                }
            }
        }
    }
}