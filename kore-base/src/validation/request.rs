// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::proof::ValidationProof;

use crate::{
    Error, 
    model::{ValueWrapper, HashId, request::EventRequest, signature::Signature},
};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use std::collections::HashSet;

/// A struct representing a validation request.
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ValidationRequest {
    pub proof: ValidationProof,
    pub subject_signature: Signature,
    pub previous_proof: Option<ValidationProof>,
    pub prev_event_validation_signatures: HashSet<Signature>,
}

