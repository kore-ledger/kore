//
//

pub mod error;
mod namespace;
mod request;
mod signature;
mod wrapper;

pub use namespace::Namespace;
pub use request::{
    CreateRequest, 
    FactRequest, 
    TransferRequest, 
    ConfirmRequest, 
    RejectRequest, 
    EOLRequest, 
    EventRequest,
};
pub use signature::Signature;
pub use signature::Signed;
pub use wrapper::ValueWrapper;

use error::Error;
use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;


/// A trait for generating a hash identifier.
pub trait HashId: BorshSerialize {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error>;
}

/// A struct representing a timestamp.
#[derive(
    Debug,
    Clone,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    BorshSerialize,
    BorshDeserialize,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct TimeStamp(pub u64);

impl TimeStamp {
    /// Returns a new `TimeStamp` representing the current time.
    pub fn now() -> Self {
        Self(OffsetDateTime::now_utc().unix_timestamp_nanos() as u64)
    }
}
