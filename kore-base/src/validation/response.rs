// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    model::{request::EventRequest, signature::Signature, HashId, TimeStamp, ValueWrapper}, Error
};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier, KeyIdentifier};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use std::collections::HashSet;

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
pub struct ValidationTimeOut {
    who: KeyIdentifier,
    re_trys: u32,
    timestamp: TimeStamp,
}

/// A Enum representing a validation response.
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
pub enum ValidationRes {
    Signature(Signature),
    TimeOut(ValidationTimeOut),
    Error(String)
}

