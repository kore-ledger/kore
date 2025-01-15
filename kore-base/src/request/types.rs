// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use crate::{
    evaluation::{request::EvaluationReq, response::EvalLedgerResponse},
    model::{
        event::{Ledger, ProtocolsSignatures},
        request::CreateRequest,
        signature::Signed,
    },
    validation::proof::ValidationProof,
    ConfirmRequest, EOLRequest, Event as KoreEvent, TransferRequest,
    ValidationInfo,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RequestManagerState {
    Reboot,
    Starting,
    Evaluation,
    Approval {
        eval_req: EvaluationReq,
        eval_res: EvalLedgerResponse,
        eval_signatures: HashSet<ProtocolsSignatures>,
    },
    Validation {
        val_info: ValidationInfo,
        last_proof: Option<ValidationProof>,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
    Distribution {
        event: Signed<KoreEvent>,
        ledger: Signed<Ledger>,
        last_proof: ValidationProof,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
}

#[allow(clippy::upper_case_acronyms)]
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

#[derive(Default)]
pub struct ProtocolsResult {
    pub eval_success: Option<bool>,
    pub appr_required: bool,
    pub appr_success: Option<bool>,
    pub eval_signatures: Option<HashSet<ProtocolsSignatures>>,
    pub appr_signatures: Option<HashSet<ProtocolsSignatures>>,
}
