use identity::{identifier::derive::KeyDerivator, keys::{Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair, KeyPairType, Secp256k1KeyPair}};
use kore_base::error::Error;
use hex_literal::hex;
use pkcs8::{pkcs5, Document, EncryptedPrivateKeyInfo, PrivateKeyInfo};

use std::fs;

use crate::config::Config;

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
                Error::Bridge(format!("Error reading node private key: {}", error))
            })?;
            let enc_pk =
                EncryptedPrivateKeyInfo::try_from(document.as_bytes()).map_err(|error| {
                    Error::Bridge(format!("Error reading node private key: {}", error))
                })?;
            let dec_pk = enc_pk.decrypt(password).map_err(|error| {
                Error::Bridge(format!("Error decrypting node private key: {}", error))
            })?;
            let key_type = match &config.kore_config.key_derivator {
                KeyDerivator::Ed25519 => KeyPairType::Ed25519,
                KeyDerivator::Secp256k1 => KeyPairType::Secp256k1,
            };
            let key_pair =
                KeyPair::from_secret_der(key_type, dec_pk.as_bytes()).map_err(|error| {
                    Error::Bridge(format!(
                        "Error creating key pair from secret der: {}",
                        error
                    ))
                })?;
            Ok(key_pair)
        }
        Err(_) => {
            let key_pair = match &config.kore_config.key_derivator {
                KeyDerivator::Ed25519 => KeyPair::Ed25519(Ed25519KeyPair::new()),
                KeyDerivator::Secp256k1 => KeyPair::Secp256k1(Secp256k1KeyPair::new()),
            };
            let der = key_pair
                .to_secret_der()
                .map_err(|error| Error::Bridge(format!("Error getting secret der: {}", error)))?;
            let pk = PrivateKeyInfo::try_from(der.as_slice()).map_err(|error| {
                Error::Bridge(format!("Error creating private key info: {}", error))
            })?;
            let params = pkcs5::pbes2::Parameters::pbkdf2_sha256_aes256cbc(
                2048,
                &hex!("79d982e70df91a88"),
                &hex!("b2d02d78b2efd9dff694cf8e0af40925"),
            )
            .map_err(|error| {
                Error::Bridge(format!("Error creating pkcs5 parameters: {}", error))
            })?;
            let enc_pk = pk
                .encrypt_with_params(params, password)
                .map_err(|_| Error::Bridge("Error encrypting private key".to_owned()))?;
            enc_pk.write_der_file(path).map_err(|error| {
                Error::Bridge(format!("Error writing node private key: {}", error))
            })?;
            Ok(key_pair)
        }
    }
}