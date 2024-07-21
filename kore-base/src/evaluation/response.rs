// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{Error, model::{ValueWrapper, HashId}};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

/// A struct representing an evaluation response.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct EvaluationResponse {
    /// The patch to apply to the state.
    pub patch: ValueWrapper,
    /// The hash of the evaluation request being responded to.
    pub eval_req_hash: DigestIdentifier,
    /// The hash of the state after applying the patch.
    pub state_hash: DigestIdentifier,
    /// Whether the evaluation was successful and the result was validated against the schema.
    pub eval_success: bool,
    /// Whether approval is required for the evaluation to be applied to the state.
    pub appr_required: bool,
}

impl HashId for EvaluationResponse {
    fn hash_id(&self, derivator: DigestDerivator) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(
            (
                &self.eval_req_hash,
                &self.state_hash,
                self.eval_success,
                self.appr_required,
            ),
            derivator,
        )
        .map_err(|_| {
            Error::Evaluation("HashId for EvaluationResponse fails".to_string())
        })
    }
}
