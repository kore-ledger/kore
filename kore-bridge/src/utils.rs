// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use identity::{
    identifier::derive::KeyDerivator,
    keys::{
        Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair, KeyPairType,
        Secp256k1KeyPair,
    },
};
use kore_base::error::Error;
use pkcs8::{Document, EncryptedPrivateKeyInfo, PrivateKeyInfo, pkcs5};

use std::fs;
use getrandom::fill;

use crate::config::Config;

const PBKDF2_ITERATIONS: u32 = 200_000;

pub fn key_pair(config: &Config, password: &str) -> Result<KeyPair, Error> {
    if fs::metadata(&config.keys_path).is_err() {
        fs::create_dir_all(&config.keys_path).map_err(|error| {
            Error::Bridge(format!("Error creating keys directory: {}", error))
        })?;
    }
    let path = format!("{}/node_private.der", &config.keys_path);
    match fs::metadata(&path) {
        Ok(_) => {
            let document = Document::read_der_file(path).map_err(|error| {
                Error::Bridge(format!(
                    "Error reading node private key: {}",
                    error
                ))
            })?;
            let enc_pk = EncryptedPrivateKeyInfo::try_from(document.as_bytes())
                .map_err(|error| {
                    Error::Bridge(format!(
                        "Error reading node private key: {}",
                        error
                    ))
                })?;
            let dec_pk = enc_pk.decrypt(password).map_err(|error| {
                Error::Bridge(format!(
                    "Error decrypting node private key: {}",
                    error
                ))
            })?;
            let key_type = match &config.kore_config.key_derivator {
                KeyDerivator::Ed25519 => KeyPairType::Ed25519,
                KeyDerivator::Secp256k1 => KeyPairType::Secp256k1,
            };
            let key_pair =
                KeyPair::from_secret_der(key_type, dec_pk.as_bytes()).map_err(
                    |error| {
                        Error::Bridge(format!(
                            "Error creating key pair from secret der: {}",
                            error
                        ))
                    },
                )?;
            Ok(key_pair)
        }
        Err(_) => {
            let key_pair = match &config.kore_config.key_derivator {
                KeyDerivator::Ed25519 => {
                    KeyPair::Ed25519(Ed25519KeyPair::new())
                }
                KeyDerivator::Secp256k1 => {
                    KeyPair::Secp256k1(Secp256k1KeyPair::new())
                }
            };
            let der = key_pair.to_secret_der().map_err(|error| {
                Error::Bridge(format!("Error getting secret der: {}", error))
            })?;
            let pk =
                PrivateKeyInfo::try_from(der.as_slice()).map_err(|error| {
                    Error::Bridge(format!(
                        "Error creating private key info: {}",
                        error
                    ))
                })?;
            let mut salt = [0u8; 32];
            let mut iv = [0u8; 16];
            fill(&mut salt).map_err(|error| {
                Error::Bridge(format!("Error generating encryption salt: {}", error))
            })?;
            fill(&mut iv).map_err(|error| {
                Error::Bridge(format!(
                    "Error generating encryption initialization vector: {}",
                    error
                ))
            })?;

            let params = pkcs5::pbes2::Parameters::pbkdf2_sha256_aes256cbc(
                PBKDF2_ITERATIONS,
                &salt,
                &iv,
            )
            .map_err(|error| {
                Error::Bridge(format!(
                    "Error creating pkcs5 parameters: {}",
                    error
                ))
            })?;
            let enc_pk =
                pk.encrypt_with_params(params, password).map_err(|_| {
                    Error::Bridge("Error encrypting private key".to_owned())
                })?;
            enc_pk.write_der_file(path).map_err(|error| {
                Error::Bridge(format!(
                    "Error writing node private key: {}",
                    error
                ))
            })?;
            Ok(key_pair)
        }
    }
}
