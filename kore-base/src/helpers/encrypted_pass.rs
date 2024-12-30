// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use generic_array::{typenum::U32, GenericArray};
use identity::identifier::derive::digest::DigestDerivator;
use memsecurity::EncryptedMem;
use serde_json::Value;
use tracing::error;

use crate::{model::{HashId, ValueWrapper}, Error, DIGEST_DERIVATOR};

const TARGET_ENCPASS: &str = "Kore-Helper-EncryptedPass";

/// Helper for encrypted password.
pub struct EncryptedPass {
    password: EncryptedMem,
}

impl EncryptedPass {
    /// Create a new `EncryptedPass`.
    pub fn new(pass: &str) -> Result<Self, Error> {
        let mut password = EncryptedMem::new();

        let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
            *derivator
        } else {
            error!(TARGET_ENCPASS, "Error getting derivator");
            DigestDerivator::Blake3_256
        };

        let value = ValueWrapper(Value::String(pass.to_owned()));
        let digest_identifier = value.hash_id(derivator).map_err(|e| Error::Password(format!("Can not generate DigestIdentifier from password: {}", e)))?;
        let digest_array = Self::vec_to_array_fixed(digest_identifier.digest);

        password.encrypt(&digest_array).map_err(|_| {
            Error::Password("Encrypt password error.".to_owned())
        })?;
        Ok(Self { password })
    }

    fn vec_to_array_fixed(vec: Vec<u8>) -> [u8; 32] {
        let mut array = [0u8; 32];
        for (i, &byte) in vec.iter().take(32).enumerate() {
            array[i] = byte;
        }
        array
    }

    pub fn key(&self) -> Option<[u8; 32]> {
        if let Ok(value) = self.password.decrypt() {
            
            let bytes: &GenericArray<u8, U32> =
                GenericArray::from_slice(value.as_ref());
            Some(bytes.into_array())
        } else {
            None
        }
    }
}

/// Clone for `EncryptedPass`
impl Clone for EncryptedPass {
    fn clone(&self) -> Self {
        let mut new = EncryptedMem::new();
        if let Some(value) = self.key() {
            new.encrypt(&value).unwrap_or(&mut EncryptedMem::new());
        }
        Self { password: new }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_encrypted_pass() {
        let ep = EncryptedPass::new("password").unwrap();
        let key1 = ep.key().unwrap();
        let ep_cloned = ep.clone();
        let key2 = ep_cloned.key().unwrap();
        assert_eq!(key1, key2);
    }
}
