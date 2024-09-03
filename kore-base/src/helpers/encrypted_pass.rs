// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2,
};
use generic_array::{typenum::U32, GenericArray};
use memsecurity::EncryptedMem;

use crate::Error;

/// Helper for encrypted password.
pub struct EncryptedPass {
    password: EncryptedMem,
}

impl EncryptedPass {
    /// Create a new `EncryptedPass`.
    pub fn new(pass: &str) -> Result<Self, Error> {
        let mut password = EncryptedMem::new();

        let salt = SaltString::generate(&mut OsRng);
        let salt = salt.as_str().as_bytes();

        let mut output_key_material = [0u8; 32]; // Can be any desired size
        Argon2::default()
            .hash_password_into(pass.as_bytes(), salt, &mut output_key_material)
            .map_err(|e| Error::Password(e.to_string()))?;

        password.encrypt(&output_key_material).map_err(|_| {
            Error::Password("Encrypt password error.".to_owned())
        })?;
        Ok(Self { password })
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
