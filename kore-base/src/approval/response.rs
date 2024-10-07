// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    model::{network::TimeOutResponse, HashId, TimeStamp},
    Error, Signature,
};
use identity::identifier::{
    derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier,
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use super::request::ApprovalReq;

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
            |_| Error::Evaluation("HashId for ApprovalRes fails".to_string()),
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
            |_| {
                Error::Evaluation(
                    "HashId for ApprovalSignature fails".to_string(),
                )
            },
        )
    }
}
