// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    governance::{Member, Policy, Role, Schema},
    ValueWrapper,
};

#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone,
)]
pub struct ContractResult {
    pub final_state: ValueWrapper,
    pub success: bool,
}

#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone,
)]
pub struct RunnerResult {
    pub final_state: ValueWrapper,
    pub approval_required: bool,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub enum Contract {
    CompiledContract(Vec<u8>),
    GovContract,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GovernanceData {
    pub version: u64,
    pub members: Vec<Member>,
    pub roles: Vec<Role>,
    pub schemas: Vec<Schema>,
    pub policies: Vec<Policy>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum GovernanceEvent {
    Patch { data: Value },
}
