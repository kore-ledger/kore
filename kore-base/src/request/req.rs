// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    db::Database,
    model::{request::EventRequest, signature::Signed},
    Error,
};

use actor::{Actor, ActorContext, Event, Handler, Message, Response};
use async_trait::async_trait;
use identity::identifier::DigestIdentifier;
use serde::{Deserialize, Serialize};

pub struct Request {
    /// The request's id.
    id: DigestIdentifier,
    /// The request's subject.
    event: Signed<EventRequest>,
}

impl Request {
    /// Creates a new `Request`.
    pub fn new(id: DigestIdentifier, event: Signed<EventRequest>) -> Self {
        Request { id, event }
    }
}

pub enum RequestState {}

#[derive(Debug, Clone)]
pub struct RequestCommand;

impl Message for RequestCommand {}

#[derive(Debug, Clone)]
pub struct RequestResponse;

impl Response for RequestResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEvent;

impl Event for RequestEvent {}

#[async_trait]
impl Actor for Request {
    type Message = RequestCommand;
    type Response = RequestResponse;
    type Event = RequestEvent;
}

#[async_trait]
impl Handler<Request> for Request {
    async fn handle_message(
        &mut self,
        msg: RequestCommand,
        ctx: &mut ActorContext<Request>,
    ) -> RequestResponse {
        RequestResponse
    }
}
