// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Digest derive
//!

use blake3;
use borsh::{BorshDeserialize, BorshSerialize};
use core::fmt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use sha3::{Sha3_256, Sha3_512};
use std::str::FromStr;

use crate::identifier::error::Error;

use super::Derivator;

/// Enumeration with digest derivator types
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
    Default,
)]
pub enum DigestDerivator {
    Blake3_256,
    #[default]
    Blake3_512,
    SHA2_256,
    SHA2_512,
    SHA3_256,
    SHA3_512,
}

impl DigestDerivator {
    pub fn digest(&self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Blake3_256 => blake3_256_digest(data),
            Self::Blake3_512 => blake3_512_digest(data),
            Self::SHA2_256 => sha2_256_digest(data),
            Self::SHA2_512 => sha2_512_digest(data),
            Self::SHA3_256 => sha3_256_digest(data),
            Self::SHA3_512 => sha3_512_digest(data),
        }
    }
}

impl fmt::Display for DigestDerivator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blake3_256 => write!(f, "Blake3_256"),
            Self::Blake3_512 => write!(f, "Blake3_512"),
            Self::SHA2_256 => write!(f, "SHA2_256"),
            Self::SHA2_512 => write!(f, "SHA2_512"),
            Self::SHA3_256 => write!(f, "SHA3_256"),
            Self::SHA3_512 => write!(f, "SHA3_512"),
        }
    }
}

impl Derivator for DigestDerivator {
    fn to_str(&self) -> String {
        match self {
            Self::Blake3_256 => "J",
            Self::Blake3_512 => "0J",
            Self::SHA2_256 => "L",
            Self::SHA2_512 => "0L",
            Self::SHA3_256 => "M",
            Self::SHA3_512 => "0M",
        }
        .into()
    }

    fn code_len(&self) -> usize {
        match self {
            Self::Blake3_256 | Self::SHA2_256 | Self::SHA3_256 => 1,
            Self::Blake3_512 | Self::SHA2_512 | Self::SHA3_512 => 2,
        }
    }

    fn derivative_len(&self) -> usize {
        match self {
            Self::Blake3_256 | Self::SHA2_256 | Self::SHA3_256 => 43,
            Self::Blake3_512 | Self::SHA2_512 | Self::SHA3_512 => 86,
        }
    }
}

impl FromStr for DigestDerivator {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &s[..1] {
            "J" => Ok(Self::Blake3_256),
            "L" => Ok(Self::SHA2_256),
            "M" => Ok(Self::SHA3_256),
            "0" => match &s[1..2] {
                "J" => Ok(Self::Blake3_512),
                "L" => Ok(Self::SHA2_512),
                "M" => Ok(Self::SHA3_512),
                _ => Err(Error::Deserialization),
            },
            _ => Err(Error::Deserialization),
        }
    }
}

/// performs blake3 256 digest
fn blake3_256_digest(input: &[u8]) -> Vec<u8> {
    blake3::hash(input).as_bytes().to_vec()
}

/// perform blake3 512 digest
fn blake3_512_digest(input: &[u8]) -> Vec<u8> {
    let mut out = [0u8; 64];
    let mut h = blake3::Hasher::new();
    h.update(input);
    h.finalize_xof().fill(&mut out);
    out.to_vec()
}

/// performs sha2 256 digest
fn sha2_256_digest(input: &[u8]) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(input);
    h.finalize().to_vec()
}

/// performs sha2 512 digest
fn sha2_512_digest(input: &[u8]) -> Vec<u8> {
    let mut h = Sha512::new();
    h.update(input);
    h.finalize().to_vec()
}

/// performs sha3 256 digest
fn sha3_256_digest(input: &[u8]) -> Vec<u8> {
    let mut h = Sha3_256::new();
    h.update(input);
    h.finalize().to_vec()
}

/// performs sha3 512 digest
fn sha3_512_digest(input: &[u8]) -> Vec<u8> {
    let mut h = Sha3_512::new();
    h.update(input);
    h.finalize().to_vec()
}

/*
impl From<DigestDerivator> for config::Value {
    fn from(data: DigestDerivator) -> Self {
        match data {
            DigestDerivator::Blake3_256 => {
                Self::new(None, config::ValueKind::String("Blake3_256".to_owned()))
            }
            DigestDerivator::Blake3_512 => {
                Self::new(None, config::ValueKind::String("Blake3_512".to_owned()))
            }
            DigestDerivator::SHA2_256 => {
                Self::new(None, config::ValueKind::String("SHA2_256".to_owned()))
            }
            DigestDerivator::SHA2_512 => {
                Self::new(None, config::ValueKind::String("SHA2_512".to_owned()))
            }
            DigestDerivator::SHA3_256 => {
                Self::new(None, config::ValueKind::String("SHA3_256".to_owned()))
            }
            DigestDerivator::SHA3_512 => {
                Self::new(None, config::ValueKind::String("SHA3_512".to_owned()))
            }
        }
    }
}
*/
