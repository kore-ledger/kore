// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    evaluation::response::EvaluationRes, governance::RequestStage, model::{
        event::Ledger, request::{CreateRequest, EventRequest}, signature::{Signature, Signed}
    }, ConfirmRequest, EOLRequest, Error, Event as KoreEvent, TransferRequest, ValidationInfo
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RequestSate {
    Starting,
    Evaluation(),
    Approval(),
    Validation(ValidationInfo),
    Distribution {
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>
    }
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

