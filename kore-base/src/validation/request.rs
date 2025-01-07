// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::proof::ValidationProof;

use crate::{
    model::{event::ProtocolsSignatures, signature::Signature, HashId},
    Error,
};

use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};
use serde::{Deserialize, Serialize};
use tracing::error;

const TARGET_REQUEST: &str = "Kore-Validation-Request";

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
    pub prev_event_validation_response: Vec<ProtocolsSignatures>,
}

impl HashId for ValidationReq {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_REQUEST, "HashId for ValidationReq fails: {}", e);
                Error::HashID(format!("HashId for ValidationReq fails: {}", e))
            },
        )
    }
}
