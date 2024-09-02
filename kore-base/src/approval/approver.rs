use actor::{Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler, Message, Response};
use async_trait::async_trait;
use identity::identifier::{derive::digest::DigestDerivator, Derivable, DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use serde::{Deserialize, Serialize};
use crate::{Error, EventRequest, NetworkMessage, Signature, Signed};

use super::request::ApprovalRequest;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Approver {
    request_id: String,
    node: KeyIdentifier,
}

fn subject_id_by_request(
    request: &EventRequest,
    gov_version: &u64,
    derivator: DigestDerivator,
) -> Result<DigestIdentifier, Error> {
    let subject_id = match request {
        EventRequest::Fact(ref fact_request) => fact_request.subject_id.clone(),
        EventRequest::Create(ref create_request) => DigestIdentifier::from_serializable_borsh(
            (
                &create_request.namespace,
                &create_request.schema_id,
                &create_request.public_key.to_str(),
                &create_request.governance_id.to_str(),
                gov_version,
                derivator,
            ),
            derivator,
        )
        .map_err(|_| Error::Approval("()".to_string()))?,
        _ => return Err(Error::Approval("Only events of type Create and Fact are allowed.".to_string())),
    };
    Ok(subject_id)
}

impl Approver {
    pub fn new(
        request_id: String,
        node: KeyIdentifier,
        ) -> Self {
        Approver { request_id, node, ..Default::default() }
    }
    async fn approval_event(&self, approval_request: Signed<ApprovalRequest>) -> Result<(), Error> {
        // Genero un id para la request
        let id = match DigestIdentifier::generate_with_blake3(&approval_request.content).map_err(
            |_| Error::Approval("".to_string()),
        ) {
            Ok(id) => id,
            Err(e) => return Err(e),
        };
        // Necesito preguntar a mi padre si tengo la request para lanzar error??¿¿

        // Obtengo el subject id de la request(FACT, CREATE(la genero))

        let subject_id = subject_id_by_request(&approval_request.content.event_request.content,&approval_request.content.gov_version, DigestDerivator::Blake3_256)?;

        // obtengo el estado del padre  para ver si tengo algo pendiente

        Ok(())
    }
}


#[derive(Debug, Clone)]
pub enum ApproverCommand {
    LocalApprover {
        approval_req: ApprovalRequest,
        our_key: KeyIdentifier,
    },
    NetworkApprover {
        request_id: String,
        approval_req: Signed<ApprovalRequest>,
        node_key: KeyIdentifier,
        our_key: KeyIdentifier,
    },
    NetworkResponse {
        approval_res: Signed<ApprovalRequest>,
        request_id: String,
    },
    NetworkRequest {
        approval_req: Signed<ApprovalRequest>,
        info: ComunicateInfo,
    },
}

impl Message for ApproverCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApproverEvent {
    AllTryHaveBeenMade { node_key: KeyIdentifier },
    ReTry(NetworkMessage),
}

impl Event for ApproverEvent {}

#[derive(Debug, Clone)]
pub enum ApproverResponse {
    None,
}

impl Response for ApproverResponse {}

#[async_trait]
impl Actor for Approver {
    type Event =ApproverEvent;
    type Message = ApproverCommand;
    type Response = ApproverResponse;
}


#[async_trait]
impl Handler<Approver> for Approver {
    async fn handle_message(
        &mut self,
        sender: ActorPath,
        msg: ApproverCommand,
        ctx: &mut ActorContext<Approver>,
    ) -> Result<ApproverResponse, ActorError> {
        match msg {
            ApproverCommand::LocalApprover { approval_req, our_key } => {
                !unimplemented!()
            }
            ApproverCommand::NetworkApprover {
                request_id,
                approval_req,
                node_key,
                our_key,
            } => {
                !unimplemented!()
            }
            ApproverCommand::NetworkResponse {
                approval_res,
                request_id,
            } => {
                !unimplemented!()
            }
            ApproverCommand::NetworkRequest {
                approval_req,
                info,
            } => {
                !unimplemented!()
            }
            
        }
    }}
    