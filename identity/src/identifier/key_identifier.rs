// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! KeyIdentifier module

use base64::{Engine as _, engine::general_purpose};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use super::{
    Derivable, SignatureIdentifier,
    derive::{Derivator, key::KeyDerivator, signature::SignatureDerivator},
    error::Error,
};
#[cfg(feature = "secp256k1")]
use crate::keys::secp256k1::Secp256k1KeyPair;
use crate::keys::{DSA, KeyGenerator, Payload, ed25519::Ed25519KeyPair};

/// Key based identifier
#[derive(
    Debug,
    Clone,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    PartialEq,
    PartialOrd,
    Ord,
)]
pub struct KeyIdentifier {
    pub public_key: Vec<u8>,

    pub derivator: KeyDerivator,
}

/// KeyIdentifier implementation
impl KeyIdentifier {
    pub fn new(derivator: KeyDerivator, pk: &[u8]) -> Self {
        Self {
            public_key: pk.to_vec(),
            derivator,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.public_key.is_empty()
    }

    pub fn to_signature_derivator(&self) -> SignatureDerivator {
        match self.derivator {
            KeyDerivator::Ed25519 => SignatureDerivator::Ed25519Sha512,
            KeyDerivator::Secp256k1 => SignatureDerivator::ECDSAsecp256k1,
        }
    }

    pub fn verify(
        &self,
        data: &[u8],
        signature: &SignatureIdentifier,
    ) -> Result<(), Error> {
        match self.derivator {
            KeyDerivator::Ed25519 => {
                let kp = Ed25519KeyPair::from_public_key(&self.public_key);
                match signature.derivator {
                    SignatureDerivator::Ed25519Sha512 => kp.verify(
                        Payload::Buffer(data.to_vec()),
                        &signature.signature,
                    ),
                    _ => Err(Error::Verification(
                        "Wrong signature type".to_owned(),
                    )),
                }
            }
            #[cfg(feature = "secp256k1")]
            KeyDerivator::Secp256k1 => {
                let kp = Secp256k1KeyPair::from_public_key(&self.public_key);
                match signature.derivator {
                    SignatureDerivator::ECDSAsecp256k1 => kp.verify(
                        Payload::Buffer(data.to_vec()),
                        &signature.signature,
                    ),
                    _ => Err(Error::Verification(
                        "Wrong signature type".to_owned(),
                    )),
                }
            }
        }
    }
}

/// KeyIdentifier for tests
impl Default for KeyIdentifier {
    fn default() -> Self {
        Self {
            public_key: vec![0; 32],
            derivator: KeyDerivator::Ed25519,
        }
    }
}

impl Display for KeyIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str(),)
    }
}

/// From string to KeyIdentifier
impl FromStr for KeyIdentifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let code = KeyDerivator::from_str(s)?;
        if s.len() == code.material_len() {
            let k_vec = general_purpose::URL_SAFE_NO_PAD
                .decode(&s[code.code_len()..code.material_len()])?;
            Ok(Self {
                derivator: code,
                public_key: k_vec,
            })
        } else {
            Err(Error::Semantic(format!(
                "Incorrect Identifier Length: {}",
                s
            )))
        }
    }
}

/// Derivable for KeyIdentifier
impl Derivable for KeyIdentifier {
    fn derivative(&self) -> Vec<u8> {
        self.public_key.clone()
    }

    fn derivation_code(&self) -> String {
        self.derivator.to_str()
    }
}

/// Serde compatible Serialize
impl Serialize for KeyIdentifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_str())
    }
}

/// Serde compatible Deserialize
impl<'de> Deserialize<'de> for KeyIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<KeyIdentifier, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s =
            <std::string::String as Deserialize>::deserialize(deserializer)?;

        KeyIdentifier::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
pub mod tests {

    use super::{Derivable, KeyIdentifier, SignatureIdentifier};
    #[cfg(feature = "secp256k1")]
    use crate::keys::secp256k1::Secp256k1KeyPair;

    use crate::keys::{
        DSA, KeyGenerator, KeyMaterial, Payload, ed25519::Ed25519KeyPair,
    };

    use crate::identifier::derive::{
        key::KeyDerivator, signature::SignatureDerivator,
    };

    use std::str::FromStr;

    #[test]
    fn test_to_from_string() {
        let key_pair = Ed25519KeyPair::new();
        let print = KeyIdentifier::new(
            KeyDerivator::Ed25519,
            &key_pair.public_key_bytes(),
        );
        let string = print.to_str();
        let from_str = KeyIdentifier::from_str(&string);
        assert!(from_str.is_ok());
        let des = from_str.unwrap();
        assert_eq!(des, print);

        #[cfg(feature = "secp256k1")]
        {
            let key_pair = Secp256k1KeyPair::new();
            let print = KeyIdentifier::new(
                KeyDerivator::Secp256k1,
                &key_pair.public_key_bytes(),
            );
            let string = print.to_str();
            let from_str = KeyIdentifier::from_str(&string);
            assert!(from_str.is_ok());
            let des = from_str.unwrap();
            assert_eq!(des, print);
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let key_pair = Ed25519KeyPair::new();
        let print = KeyIdentifier::new(
            KeyDerivator::Ed25519,
            &key_pair.public_key_bytes(),
        );
        let ser = serde_json::to_string(&print);
        assert!(ser.is_ok());
        let des: Result<KeyIdentifier, _> = serde_json::from_str(&ser.unwrap());
        assert!(des.is_ok());
    }

    #[test]
    fn test_verify_ed25519() {
        let kp = Ed25519KeyPair::new();
        let message = b"message";
        let sig = kp.sign(Payload::Buffer(message.to_vec())).unwrap();
        let id =
            KeyIdentifier::new(KeyDerivator::Ed25519, &kp.public_key_bytes());
        let signature =
            SignatureIdentifier::new(SignatureDerivator::Ed25519Sha512, &sig);
        assert!(id.verify(message, &signature).is_ok());
    }

    #[test]
    #[cfg(feature = "secp256k1")]
    fn test_verify_secp256k1() {
        let kp = Secp256k1KeyPair::new();
        let message = b"message";
        let sig = kp.sign(Payload::Buffer(message.to_vec())).unwrap();
        let id =
            KeyIdentifier::new(KeyDerivator::Secp256k1, &kp.public_key_bytes());
        let signature =
            SignatureIdentifier::new(SignatureDerivator::ECDSAsecp256k1, &sig);
        assert!(id.verify(message, &signature).is_ok());
    }
}
