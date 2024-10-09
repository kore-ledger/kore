// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{HashId, TimeStamp};

use crate::Error;

use identity::{
    identifier::{
        derive::digest::DigestDerivator, Derivable, DigestIdentifier,
        KeyIdentifier, SignatureIdentifier,
    },
    keys::{KeyMaterial, KeyPair, Payload, DSA},
};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use std::hash::Hash;

/// The format, in addition to the signature, includes additional
/// information, namely the signer's identifier, the signature timestamp
/// and the hash of the signed contents.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
    Ord,
    PartialEq,
    Hash,
)]
pub struct Signature {
    /// Signer identifier
    pub signer: KeyIdentifier,
    /// Timestamp of the signature
    pub timestamp: TimeStamp,
    /// Hash of the content signed
    pub content_hash: DigestIdentifier,
    /// The signature itself
    pub value: SignatureIdentifier,
}

impl Signature {
    /// It allows the creation of a new signature
    /// # Arguments
    /// - content: The content to sign
    /// - keys: The [KeyPair] to use to generate the signature
    pub fn new<T: HashId>(
        content: &T,
        keys: &KeyPair,
        derivator: DigestDerivator,
    ) -> Result<Self, Error> {
        let signer = KeyIdentifier::new(
            keys.get_key_derivator(),
            &keys.public_key_bytes(),
        );
        let timestamp = TimeStamp::now();
        // TODO: Analyze if we should remove HashId and change it for BorshSerialize
        // let content_hash = content.hash_id()?;
        let signature_hash = DigestIdentifier::from_serializable_borsh(
            (&content, &timestamp),
            derivator,
        )
        .map_err(|_| Error::Signature("Signature hash fails".to_string()))?;
        let signature = keys
            .sign(Payload::Buffer(signature_hash.derivative()))
            .map_err(|_| Error::Signature("Keys sign fails".to_owned()))?;
        Ok(Signature {
            signer: signer.clone(),
            timestamp,
            content_hash: signature_hash,
            value: SignatureIdentifier::new(
                signer.to_signature_derivator(),
                &signature,
            ),
        })
    }

    /// It allow verify the signature. It checks if the content and the signer are correct
    pub fn verify<T: HashId>(&self, content: &T) -> Result<(), Error> {
        let derivator = self.content_hash.derivator;
        // let content_hash = content.hash_id(derivator)?;
        let signature_hash = DigestIdentifier::from_serializable_borsh(
            (&content, &self.timestamp),
            derivator,
        )
        .map_err(|_| Error::Signature("Signature hash fails".to_string()))?;
        self.signer
            .verify(&signature_hash.digest, &self.value)
            .map_err(|_| Error::Signature("Signature verify fails".to_owned()))
    }
}

/// Represents any signed data entity
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
    Hash,
)]
pub struct Signed<T: BorshSerialize + BorshDeserialize + Clone + HashId> {
    /// The data that is signed
    pub content: T,
    /// The signature accompanying the data
    pub signature: Signature,
}

impl<T: BorshSerialize + BorshDeserialize + Clone + HashId> Signed<T> {
    /// It allows the creation of a new signed entity
    /// # Arguments
    /// - content: The content to sign
    /// - keys: The [KeyPair] to use to generate the signature
    pub fn new(content: T, signature: Signature) -> Self {
        Signed { content, signature }
    }

    /// It allow verify the signature. It checks if the content and the signer are correct
    pub fn verify(&self) -> Result<(), Error> {
        self.signature.verify(&self.content)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use identity::{
        identifier::derive::digest::DigestDerivator,
        keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
    };

    // Generate key pair for testing
    pub fn generate_key_pair() -> KeyPair {
        let kp = Ed25519KeyPair::new();
        KeyPair::Ed25519(kp)
    }

    // Generate signature for testing
    pub fn generate_signature<T: HashId>(
        content: &T,
        key_pair: &KeyPair,
    ) -> Signature {
        Signature::new(content, key_pair, DigestDerivator::SHA2_256).unwrap()
    }

    #[derive(
        Debug,
        Clone,
        Serialize,
        Deserialize,
        BorshSerialize,
        BorshDeserialize,
        PartialEq,
    )]
    struct TestContent {
        pub value: String,
    }

    impl HashId for TestContent {
        fn hash_id(
            &self,
            derivator: DigestDerivator,
        ) -> Result<DigestIdentifier, Error> {
            DigestIdentifier::from_serializable_borsh(self, derivator)
                .map_err(|_| Error::Signature("Hash fails".to_string()))
        }
    }

    #[test]
    fn test_signature() {
        let key_pair = generate_key_pair();
        let content = TestContent {
            value: "test".to_owned(),
        };
        let signature = generate_signature(&content, &key_pair);
        assert!(signature.verify(&content).is_ok());
    }

    #[test]
    fn test_signed() {
        let key_pair = generate_key_pair();
        let content = TestContent {
            value: "test".to_owned(),
        };
        let signature = generate_signature(&content, &key_pair);
        let signed = Signed { content, signature };
        assert!(signed.signature.verify(&signed.content).is_ok());
    }
}
