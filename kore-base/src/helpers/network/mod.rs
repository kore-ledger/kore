use actor::Message;
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};

use crate::validation::{request::ValidationReq, response::ValidationRes};

pub mod intermediary;
pub mod service;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ActorMessage {
    ValidationReq(ValidationReq),
    ValidationRes(ValidationRes),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkMessage {
    pub info: ComunicateInfo,
    pub message: ActorMessage,
}

impl Message for NetworkMessage {}