// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{
    proof::ValidationProof,
    response::{ValidationRes, ValidationTimeOut},
};

use crate::{
    model::{
        request::EventRequest, signature::Signature, HashId, ValueWrapper,
    },
    Error,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// A struct representing a validation request.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct ValidationReq {
    // La generamos nosotros
    pub proof: ValidationProof,
    // La generamos nosotros, keypair, derivator (del sujeto) Lo tiene que generar el sujeto
    pub subject_signature: Signature,
    // Hay que sacarlo de la base de datos,
    pub previous_proof: Option<ValidationProof>,
    // Hay que sacarlo de la base de datos,
    pub prev_event_validation_response: Vec<SignersRes>,
}

/// Accept response of Validators, can be a Signature or a TimeOut if all trys have been made
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub enum SignersRes {
    Signature(Signature),
    TimeOut(ValidationTimeOut),
}
