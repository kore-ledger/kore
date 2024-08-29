use identity::identifier::{derive::digest::DigestDerivator, Derivable, DigestIdentifier};
use serde::{Deserialize, Serialize};
use crate::{Error, EventRequest, Signature, Signed};

use super::request::ApprovalRequest;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    request_id: String,
    finish: bool,
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

impl Validator {
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