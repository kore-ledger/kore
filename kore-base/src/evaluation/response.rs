

use crate::{
    Error,
    model::{
        HashId, ValueWrapper, event::LedgerValue, network::TimeOutResponse,
    },
};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use tracing::error;

const TARGET_RESPONSE: &str = "Kore-Evaluation-Response";

/// A struct representing an evaluation response.
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
pub enum EvaluationRes {
    Error(String),
    TimeOut(TimeOutResponse),
    Response(Response),
    Reboot,
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
    Hash,
)]
pub struct Response {
    /// The patch to apply to the state.
    pub patch: ValueWrapper,
    /// The hash of the state after applying the patch.
    pub state_hash: DigestIdentifier,
    /// Whether approval is required for the evaluation to be applied to the state.
    pub appr_required: bool,
}

impl HashId for EvaluationRes {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_RESPONSE, "HashId for ProofEvent fails: {}", e);
                Error::HashID(format!("HashId for ProofEvent fails: {}", e))
            },
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalLedgerResponse {
    /// The patch to apply to the state.
    pub value: LedgerValue,
    /// The hash of the state after applying the patch.
    pub state_hash: DigestIdentifier,
    /// Whether the evaluation was successful and the result was validated against the schema.
    pub eval_success: bool,
    /// Whether approval is required for the evaluation to be applied to the state.
    pub appr_required: bool,
}

impl From<Response> for EvalLedgerResponse {
    fn from(value: Response) -> Self {
        Self {
            value: LedgerValue::Patch(value.patch),
            state_hash: value.state_hash,
            eval_success: true,
            appr_required: value.appr_required,
        }
    }
}
