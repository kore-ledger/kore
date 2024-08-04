use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::validation::{request::ValidationReq, response::ValidationRes};

pub mod service;
pub mod intermediary;

#[derive(Debug, Serialize, Deserialize)]
pub enum ActorMessage {
    ValidationReq(ValidationReq),
    ValidationRes(ValidationRes)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkMessage {
    info: ComunicateInfo,
    message: ActorMessage
}
