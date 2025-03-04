// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Key derivation module
//!

use borsh::{BorshDeserialize, BorshSerialize};
use core::fmt;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::{Derivator, SignatureDerivator};
use crate::identifier::{error::Error, key_identifier::KeyIdentifier};

/// An enumeration of key derivator types.
#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
    Ord,
)]
pub enum KeyDerivator {
    /// The Ed25519 key derivator.
    Ed25519,
    /// The Secp256k1 key derivator.
    Secp256k1,
}

impl KeyDerivator {
    pub fn derive(&self, public_key: &[u8]) -> KeyIdentifier {
        KeyIdentifier::new(*self, public_key)
    }

    pub fn to_signature_derivator(&self) -> SignatureDerivator {
        match self {
            KeyDerivator::Ed25519 => SignatureDerivator::Ed25519Sha512,
            KeyDerivator::Secp256k1 => SignatureDerivator::ECDSAsecp256k1,
        }
    }
}

impl fmt::Display for KeyDerivator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyDerivator::Ed25519 => write!(f, "Ed25519"),
            KeyDerivator::Secp256k1 => write!(f, "Secp256k1"),
        }
    }
}

impl Derivator for KeyDerivator {
    fn code_len(&self) -> usize {
        match self {
            Self::Ed25519 | Self::Secp256k1 => 1,
        }
    }

    fn derivative_len(&self) -> usize {
        match self {
            Self::Ed25519 => 43,
            Self::Secp256k1 => 44,
        }
    }

    fn to_str(&self) -> String {
        match self {
            Self::Ed25519 => "E",
            Self::Secp256k1 => "S",
        }
        .into()
    }
}

impl FromStr for KeyDerivator {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(Error::Deserialization);
        }
        match &s[..1] {
            "E" => Ok(Self::Ed25519),
            "S" => Ok(Self::Secp256k1),
            _ => Err(Error::Deserialization),
        }
    }
}

/*
impl From<KeyDerivator> for config::Value {
    fn from(data: KeyDerivator) -> Self {
        match data {
            KeyDerivator::Ed25519 => {
                Self::new(None, config::ValueKind::String("Ed25519".to_owned()))
            }
            KeyDerivator::Secp256k1 => {
                Self::new(None, config::ValueKind::String("Secp256k1".to_owned()))
            }
        }
    }
}
*/
