// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! API module
//!

use crate::{
    model::{request::EventRequest, signature::Signed},
    node::Node,
    request::{RequestHandler, RequestHandlerResponse},
    Error,
};

use actor::{Actor, ActorContext, ActorPath, ActorRef, Handler, SystemRef};
use identity::{identifier::KeyIdentifier, keys::KeyPair};

pub struct Api {
    keys: KeyPair,
    request: ActorRef<RequestHandler>,
}

impl Api {
    /// Creates a new `Api`.
    pub fn new(keys: KeyPair, request: ActorRef<RequestHandler>) -> Self {
        Self { keys, request }
    }

    /// Request from issuer.
    pub async fn external_request(
        &self,
        event: Signed<EventRequest>,
    ) -> Result<RequestHandlerResponse, Error> {
        /*
        self.request
            .ask(RequestHandlerCommand::StartRequest(event))
            .await
            .map_err(|e| Error::Actor(e.to_string()))
         */
        todo!()
    }

    /// Own request.
    pub async fn own_request(
        &self,
        event: EventRequest,
    ) -> Result<RequestHandlerResponse, Error> {
        todo!()
    }
}
