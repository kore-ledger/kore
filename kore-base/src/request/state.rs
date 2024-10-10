// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    evaluation::response::EvaluationRes, governance::RequestStage, model::{
        request::{CreateRequest, EventRequest},
        signature::{Signature, Signed},
    }, ConfirmRequest, EOLRequest, Error, TransferRequest, ValidationInfo
};

use actor::{Actor, ActorContext, Event, Handler, Message, Response};
use async_trait::async_trait;
use identity::identifier::DigestIdentifier;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RequestSate {
    Starting,
    Evaluation(),
    Approval(),
    Validation(ValidationInfo),
    Distribution()
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValidationProtocols {
        /// A request to create a new subject.
        Create(CreateRequest),
        /// A request to transfer ownership of a subject.
        Transfer(TransferRequest),

        Confirm(ConfirmRequest),
        /// A request to mark a subject as end-of-life.
        EOL(EOLRequest),
}

