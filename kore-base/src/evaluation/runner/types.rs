// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::KeyIdentifier;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ValueWrapper, model::Namespace};

#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone,
)]
pub struct ContractResult {
    pub final_state: ValueWrapper,
    pub success: bool,
    pub error: String,
}

#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone,
)]
pub struct RunnerResult {
    pub final_state: ValueWrapper,
    pub approval_required: bool,
}

#[derive(Debug, Clone)]
pub enum EvaluateType {
    NotGovFact {
        contract: Vec<u8>,
        payload: ValueWrapper,
    },
    GovFact {
        payload: ValueWrapper,
    },
    GovTransfer {
        new_owner: KeyIdentifier,
    },
    NotGovTransfer {
        new_owner: KeyIdentifier,
        namespace: Namespace,
        schema_id: String,
    },
    GovConfirm {
        new_owner: KeyIdentifier,
        old_owner_name: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum GovernancePatch {
    Patch { data: Value },
}
