

use crate::{
    Error,
    model::{HashId, network::TimeOutResponse, signature::Signature},
};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use tracing::error;

const TARGET_RESPONSE: &str = "Kore-Validation-Response";

/// A Enum representing a validation response.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
)]
pub enum ValidationRes {
    Signature(Signature),
    TimeOut(TimeOutResponse),
    Error(String),
    Reboot,
}

impl HashId for ValidationRes {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(
                    TARGET_RESPONSE,
                    "HashId for ValidationRes fails: {}", e
                );
                Error::HashID(format!("HashId for ValidationRes fails: {}", e))
            },
        )
    }
}
