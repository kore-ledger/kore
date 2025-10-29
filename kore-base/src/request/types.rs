use std::collections::HashSet;

use crate::{
    Event as KoreEvent, ValidationInfo,
    evaluation::{request::EvaluationReq, response::EvalLedgerResponse},
    model::{
        event::{Ledger, ProtocolsSignatures},
        signature::Signed,
    },
    validation::proof::ValidationProof,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RequestManagerState {
    Reboot,
    Starting,
    Evaluation,
    Approval {
        eval_req: Box<EvaluationReq>,
        eval_res: EvalLedgerResponse,
        eval_signatures: HashSet<ProtocolsSignatures>,
    },
    Validation {
        val_info: Box<ValidationInfo>,
        last_proof: Option<ValidationProof>,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
    Distribution {
        event: Box<Signed<KoreEvent>>,
        ledger: Box<Signed<Ledger>>,
        last_proof: ValidationProof,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReqManInitMessage {
    Evaluate,
    Validate,
}

#[derive(Default)]
pub struct ProtocolsResult {
    pub eval_success: Option<bool>,
    pub appr_required: bool,
    pub appr_success: Option<bool>,
    pub eval_signatures: Option<HashSet<ProtocolsSignatures>>,
    pub appr_signatures: Option<HashSet<ProtocolsSignatures>>,
}
