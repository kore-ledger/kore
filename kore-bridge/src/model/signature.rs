// Copyright 2024 Kore Ledger
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Signature model.
//!

use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{Derivable, DigestIdentifier, KeyIdentifier, SignatureIdentifier};
use kore_base::{error::Error, model::{signature::{Signature, Signed}, HashId, TimeStamp}};
use serde::{Deserialize, Serialize};

use std::{fmt::Debug, str::FromStr};

/// Signature model.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BridgeSignature {
    /// Public key of the issuer
    signer: String, // KeyIdentifier
    /// Timestamp at which the signature was made
    timestamp: u64,
    /// Signature value
    value: String, // SignatureIdentifier,
    /// Content hash
    content_hash: String,
}

impl From<Signature> for BridgeSignature {
    fn from(signature: Signature) -> Self {
        Self {
            signer: signature.signer.to_str(),
            timestamp: signature.timestamp.0,
            value: signature.value.to_str(),
            content_hash: signature.content_hash.to_str(),
        }
    }
}

impl TryFrom<BridgeSignature> for Signature {
    type Error = Error;
    fn try_from(signature: BridgeSignature) -> Result<Self, Self::Error> {
        Ok(Self {
            signer: KeyIdentifier::from_str(&signature.signer)
                .map_err(|_| Error::Bridge("key identifier".to_owned()))?,
            timestamp: TimeStamp(signature.timestamp),
            value: SignatureIdentifier::from_str(&signature.value)
                .map_err(|_| Error::Bridge("signature identifier".to_owned()))?,
            content_hash: DigestIdentifier::from_str(&signature.content_hash)
                .map_err(|_| Error::Bridge("digest identifier".to_owned()))?,
        })
    }
}

/// Signed content.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BridgeSigned<T>
where
    T: Clone + Debug,
{
    /// Content
    #[serde(flatten)]
    pub content: T,
    /// Signature
    pub signature: BridgeSignature,
}

impl<C, T> From<Signed<C>> for BridgeSigned<T>
where
    C: BorshDeserialize + BorshSerialize + Clone + Debug + HashId,
    T: From<C> + Clone + Debug,
{
    fn from(signed: Signed<C>) -> Self {
        Self {
            content: signed.content.into(),
            signature: signed.signature.into(),
        }
    }
}

impl<C, T> TryFrom<BridgeSigned<T>> for Signed<C>
where
    C: BorshDeserialize + BorshSerialize + Clone + Debug + HashId,
    T: Into<C> + Clone + Debug,
{
    type Error = Error;
    fn try_from(signed: BridgeSigned<T>) -> Result<Self, Error> {
        Ok(Self {
            content: signed.content.into(),
            signature: signed.signature.try_into()?,
        })
    }
}
