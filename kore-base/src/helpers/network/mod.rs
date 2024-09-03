use actor::Message;
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use crate::{
    evaluation::{request::EvaluationReq, response::EvaluationRes},
    validation::{request::ValidationReq, response::ValidationRes},
    Signed,
};

pub mod intermediary;
pub mod service;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ActorMessage {
    ValidationReq(Signed<ValidationReq>),
    ValidationRes(Signed<ValidationRes>),
    EvaluationReq(Signed<EvaluationReq>),
    EvaluationRes(Signed<EvaluationRes>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkMessage {
    pub info: ComunicateInfo,
    pub message: ActorMessage,
}

impl Message for NetworkMessage {}
