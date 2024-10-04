use actor::Message;
use identity::{
    identifier::{DigestIdentifier, KeyIdentifier},
    keys::KeyPair,
};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use crate::{
    approval::{request::ApprovalReq, response::ApprovalRes, Approval}, evaluation::{request::EvaluationReq, response::EvaluationRes}, model::event::Ledger, validation::{request::ValidationReq, response::ValidationRes}, Event as KoreEvent, Signed
};

pub mod intermediary;
pub mod service;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ActorMessage {
    ValidationReq {
        req: Signed<ValidationReq>,
    },
    ValidationRes {
        res: Signed<ValidationRes>,
    },
    EvaluationReq {
        req: Signed<EvaluationReq>,
    },
    EvaluationRes {
        res: Signed<EvaluationRes>,
    },
    ApprovalReq {
        req: Signed<ApprovalReq>,
    },
    ApprovalRes {
        res: Signed<ApprovalRes>,
    },
    DistributionLastEventReq {
        ledger: Signed<Ledger>,
        event: Signed<KoreEvent>,
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
        last_event: Option<Signed<KoreEvent>>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkMessage {
    pub info: ComunicateInfo,
    pub message: ActorMessage,
}

impl Message for NetworkMessage {}
