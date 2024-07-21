// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    Error, 
    model::{ValueWrapper, HashId, request::EventRequest, signature::Signature},
};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use std::collections::HashSet;

/// A struct representing a validation response.
/// A struct representing a validation response.
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
pub struct ValidationResponse {
    pub validation_signature: Signature,
    pub gov_version_validation: u64,
}

