// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use ed25519_dalek::ed25519;
use std::convert::Infallible;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Borsh serialization failed")]
    BorshSerializationFailed,

    #[error("Errors that can never happen")]
    Infalible {
        #[from]
        source: Infallible,
    },

    #[error("Verification error: {0}")]
    Verification(String),

    #[error("`{0}`")]
    Payload(String),

    #[error("Deserialization error")]
    Deserialization,

    #[error("Base58 Decoding error")]
    Base64Decoding {
        #[from]
        source: base64::DecodeError,
    },

    #[error("Ed25519 error")]
    Ed25519 {
        #[from]
        source: ed25519::Error,
    },

    #[error("Serde JSON error")]
    SerdeJson {
        #[from]
        source: serde_json::Error,
    },

    #[error("MessagePack serialize error")]
    MsgPackSerialize {
        #[from]
        source: rmp_serde::encode::Error,
    },

    #[error("MessagePack deserialize error")]
    MsgPackDeserialize {
        #[from]
        source: rmp_serde::decode::Error,
    },

    #[error("Seed error: {0}")]
    SeedError(String),

    #[error("Semantic error: {0}")]
    Semantic(String),

    #[error("Sign error: {0}")]
    Sign(String),

    #[error("Key pair error: {0}")]
    KeyPair(String),

    #[error("Kore error: {0}")]
    Kore(String),
}
