// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::ValidationInfo;

use crate::{
    Error, DIGEST_DERIVATOR,
    model::{Namespace, HashId, request::EventRequest},
};
use identity::identifier::{DigestIdentifier, KeyIdentifier, derive::digest::DigestDerivator};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use tracing::{debug, error};

/// A struct representing a validator actor.
pub struct Validator {

}