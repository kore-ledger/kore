use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use rush::Message;
use serde::{Deserialize, Serialize};

use crate::{
    Event as KoreEvent, Signed,
    approval::{request::ApprovalReq, response::ApprovalRes},
    evaluation::{request::EvaluationReq, response::EvaluationRes},
    model::event::{Ledger, ProtocolsSignatures},
    update::TransferResponse,
    validation::{
        proof::ValidationProof, request::ValidationReq, response::ValidationRes,
    },
};

pub mod intermediary;
pub mod service;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ActorMessage {
    ValidationReq {
        req: Box<Signed<ValidationReq>>,
        schema_id: String,
    },
    ValidationRes {
        res: Signed<ValidationRes>,
    },
    EvaluationReq {
        req: Signed<EvaluationReq>,
        schema_id: String,
    },
    EvaluationRes {
        res: Signed<EvaluationRes>,
    },
    ApprovalReq {
        req: Signed<ApprovalReq>,
    },
    ApprovalRes {
        res: Box<Signed<ApprovalRes>>,
    },
    DistributionLastEventReq {
        ledger: Box<Signed<Ledger>>,
        event: Box<Signed<KoreEvent>>,
        last_proof: ValidationProof,
        prev_event_validation_response: Vec<ProtocolsSignatures>,
    },
    DistributionLastEventRes {
        signer: KeyIdentifier,
    },
    DistributionLedgerReq {
        gov_version: Option<u64>,
        actual_sn: Option<u64>,
        subject_id: DigestIdentifier,
    },
    DistributionLedgerRes {
        ledger: Vec<Signed<Ledger>>,
        last_event: Box<Option<Signed<KoreEvent>>>,
        last_proof: Option<ValidationProof>,
        namespace: String,
        schema_id: String,
        governance_id: DigestIdentifier,
        prev_event_validation_response: Option<Vec<ProtocolsSignatures>>,
    },
    DistributionGetLastSn {
        subject_id: DigestIdentifier,
    },
    AuthLastSn {
        sn: u64,
    },
    Transfer {
        subject_id: DigestIdentifier,
    },
    TransferRes {
        res: TransferResponse,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkMessage {
    pub info: ComunicateInfo,
    pub message: ActorMessage,
}

impl Message for NetworkMessage {}
