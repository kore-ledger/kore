// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    model::{network::TimeOutResponse, HashId},
    Error, Signature,
};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use tracing::error;

use super::request::ApprovalReq;

const TARGET_RESPONSE: &str = "Kore-Approval-Response";

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
)]
pub enum ApprovalRes {
    Response(Signature, bool),
    TimeOut(TimeOutResponse),
}

impl HashId for ApprovalRes {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_RESPONSE, "HashId for ApprovalRes fails: {}", e);
                Error::HashID(format!("HashId for ApprovalRes fails: {}", e))
            },
        )
    }
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
)]
pub struct ApprovalSignature {
    pub request: ApprovalReq,
    pub response: bool,
}

impl HashId for ApprovalSignature {
    fn hash_id(
        &self,
        derivator: DigestDerivator,
    ) -> Result<DigestIdentifier, Error> {
        DigestIdentifier::from_serializable_borsh(self, derivator).map_err(
            |e| {
                error!(TARGET_RESPONSE, "HashId for ApprovalSignature fails: {}", e);
                Error::HashID(format!("HashId for ApprovalSignature fails: {}", e))
            },
        )
    }
}
