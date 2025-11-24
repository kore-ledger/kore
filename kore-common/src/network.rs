//

use crate::request::{EventRequest, RequestHandlerResponse};
use crate::signature::Signed;

use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};

/// Actor message enumeration for the Network service.
/// 
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ActorMessage {
    /// Event request message.
    EventReq {
        request: Signed<EventRequest>,
    },
    /// Event response message.
    EventRes {
        response: RequestHandlerResponse,
    },
}

/// Event enumeration for the Helper service.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComunicateInfo {
    /// The request id.
    pub request_id: String,
    /// The request version.
    pub version: u64,
    /// The sender key identifier.
    pub sender: KeyIdentifier,
    /// The receiver key identifier.
    pub receiver: KeyIdentifier,
    /// The receiver actor.
    pub receiver_actor: String,
}

/// Network message struct combining communication info and actor message.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkMessage {
    /// The communication info.
    pub info: ComunicateInfo,
    /// The actor message.
    pub message: ActorMessage,
}
