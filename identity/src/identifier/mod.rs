// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Identifiers module
//!

pub mod derive;
pub mod digest_identifier;
pub mod error;
pub mod key_identifier;
pub mod signature_identifier;

pub use digest_identifier::DigestIdentifier;
pub use key_identifier::KeyIdentifier;
pub use signature_identifier::SignatureIdentifier;

use base64::{Engine as _, engine::general_purpose};
use std::str::FromStr;

use self::error::Error;

/// Derivable Identifiers
pub trait Derivable: FromStr<Err = Error> {
    fn derivative(&self) -> Vec<u8>;

    fn derivation_code(&self) -> String;

    fn to_str(&self) -> String {
        match self.derivative().len() {
            0 => "".to_string(),
            _ => [
                self.derivation_code(),
                general_purpose::URL_SAFE_NO_PAD.encode(self.derivative()),
            ]
            .join(""),
        }
    }
}
