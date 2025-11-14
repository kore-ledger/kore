//

use crate::{ValueWrapper, Namespace, Error, HashId, Signed};
use identity::identifier::{
    DigestIdentifier, KeyIdentifier, derive::digest::DigestDerivator,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use tracing::error;

const TARGET_REQUEST: &str = "Kore-Request";

// Only for internal processing in Kore Ledger
/* 
/// # Signer Types
/// An enumeration of the different types of signers.
pub enum SignerTypes {
    One(KeyIdentifier),
    List(HashSet<KeyIdentifier>, bool),
}

/// # Event Request Types
/// An enumeration of the different types of event requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventRequestType {
    Create,
    Fact,
    Transfer,
    Confirm,
    Reject,
    EOL,
}
*/

/// An enum representing a Kore Ledger event request.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub enum EventRequest {
    /// A request to create a new subject.
    Create(CreateRequest),
    /// A request to add a fact to a subject.
    Fact(FactRequest),
    /// A request to transfer ownership of a subject.
    Transfer(TransferRequest),

    Confirm(ConfirmRequest),

    Reject(RejectRequest),
    /// A request to mark a subject as end-of-life.
    EOL(EOLRequest),
}

/// A struct representing a request to create a new subject.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct CreateRequest {
    /// The name of subject.
    pub name: Option<String>,
    /// The description of subject.
    pub description: Option<String>,
    /// The identifier of the governance contract.
    pub governance_id: DigestIdentifier,
    /// The identifier of the schema used to validate the event.
    pub schema_id: String,
    /// The namespace of the subject.
    pub namespace: Namespace,
}

/// A struct representing a request to add a fact to a subject.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct FactRequest {
    /// The identifier of the subject to which the fact will be added.
    pub subject_id: DigestIdentifier,
    /// The payload of the fact to be added.
    pub payload: ValueWrapper,
}

/// A struct representing a request to transfer ownership of a subject.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct TransferRequest {
    /// The identifier of the subject to transfer ownership of.
    pub subject_id: DigestIdentifier,
    /// The identifier of the public key of the new owner.
    pub new_owner: KeyIdentifier,
}

/// A struct representing a request to transfer ownership of a subject.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct ConfirmRequest {
    pub subject_id: DigestIdentifier,
    /// The new name of old owner, only for governance confirm, if is None in governance confirm, old owner will not add to members
    pub name_old_owner: Option<String>,
}

/// A struct representing a request to mark a subject as end-of-life.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct EOLRequest {
    /// The identifier of the subject to mark as end-of-life.
    pub subject_id: DigestIdentifier,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct RejectRequest {
    pub subject_id: DigestIdentifier,
}

impl HashId for EventRequest {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_REQUEST, "HashId for EventRequest fails: {}", e);
                Error::HashID(format!("HashId for EventRequest fails: {}", e))
            },
        )
    }
}

impl HashId for Signed<EventRequest> {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(
                    TARGET_REQUEST,
                    "HashId for Signed<EventRequest> fails: {}", e
                );
                Error::HashID(format!(
                    "HashId for Signed<EventRequest> fails: {}",
                    e
                ))
            },
        )
    }
}

/* State of Even Request only for internal processing in Kore Ledger
/// Indicates the current status of an event request.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
)]
pub enum RequestState {
    Finished,
    Error,
    Processing,
}
*/

#[cfg(test)]
mod tests {

    use super::*;

    use identity::identifier::derive::digest::DigestDerivator;

    #[test]
    fn test_event_request_hash_id() {
        let mut ns = Namespace::new();
        ns.add("test");
        let create_request = CreateRequest {
            name: Some("Test Subject".to_string()),
            description: Some("This is a test subject.".to_string()),
            governance_id: DigestIdentifier::default(),
            schema_id: "kore:test:schema:1.0".to_string(),
            namespace: ns,
        };

        let event_request = EventRequest::Create(create_request);

        let derivator = DigestDerivator::SHA2_256;

        let hash_id_result = event_request.hash_id(derivator);

        assert!(hash_id_result.is_ok());

    }
}