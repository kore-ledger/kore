// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    evaluation::response::EvaluationRes,
    governance::RequestStage,
    model::{
        request::{CreateRequest, EventRequest},
        signature::{Signature, Signed},
    },
    Error,
};

use actor::{Actor, ActorContext, Event, Handler, Message, Response};
use async_trait::async_trait;
use identity::identifier::DigestIdentifier;
use serde::{Deserialize, Serialize};

/// Request state actor.
/// This actor manages the state of a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestState {
    id: DigestIdentifier,
    request: Option<EventRequest>,
    stage: RequestStage,
}

impl RequestState {
    // Creates a new `RequestState`.
    /*
    pub fn from_request(id: DigestIdentifier, request: EventRequest) -> Self {
        Self {
            id,
            request: Some(request),
            stage: RequestStage::Create,
        }
    }
    */
}

/*
#[derive(Clone, Debug)]
pub enum StateCommand {
    Evaluation(EvaluationRes),
    Approval(ApprovalResponse),
    Validation(Signature),
}
*/
