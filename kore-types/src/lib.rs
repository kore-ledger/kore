


pub mod request;
pub mod signature;
pub mod error;
pub mod wrapper;

use borsh::{BorshDeserialize, BorshSerialize};
use error::Error;
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};
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
