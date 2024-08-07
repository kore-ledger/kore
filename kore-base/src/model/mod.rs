// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Data model for Kore base.
//!
//! This module contains the data model for the Kore base.
//!

pub mod event;
pub mod namespace;
pub mod request;
pub mod signature;
pub mod wrapper;

pub use namespace::Namespace;
pub use wrapper::ValueWrapper;

use crate::{validation::proof::ValidationProof, Error};

use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use std::hash::Hash;

// TODO: Separar los tipos de firma, que el sujeto tenga los suyos y el nodo los suyos, solo en caso de que sean muy diferentes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SignTypes {
    Validation(ValidationProof),
}

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
