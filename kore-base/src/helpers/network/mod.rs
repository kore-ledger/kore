use actor::Message;
use identity::keys::KeyPair;
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use crate::{
    evaluation::{request::EvaluationReq, response::EvaluationRes}, model::event::Ledger, validation::{request::ValidationReq, response::ValidationRes}, Event as KoreEvent, Signed
};

pub mod intermediary;
pub mod service;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ActorMessage {
    ValidationReq(Signed<ValidationReq>),
    ValidationRes(Signed<ValidationRes>),
    EvaluationReq(Signed<EvaluationReq>),
    EvaluationRes(Signed<EvaluationRes>),
    DistributionLastEventReq(Signed<KoreEvent>, Option<KeyPair>),
    DistributionLedgerRes(Vec<Signed<Ledger>>, Option<KeyPair>)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkMessage {
    pub info: ComunicateInfo,
    pub message: ActorMessage,
}

impl Message for NetworkMessage {}
