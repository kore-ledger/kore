// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use base64::{Engine as _, engine::general_purpose};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use super::{
    Derivable,
    derive::{Derivator, signature::SignatureDerivator},
    error::Error,
};

/// Signature based identifier
#[derive(
    Debug,
    PartialEq,
    Clone,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    PartialOrd,
    Ord,
)]
pub struct SignatureIdentifier {
    pub derivator: SignatureDerivator,
    pub signature: Vec<u8>,
}

impl SignatureIdentifier {
    pub fn new(derivator: SignatureDerivator, signature: &[u8]) -> Self {
        Self {
            derivator,
            signature: signature.to_vec(),
        }
    }
}

/// SignagtureIdentifier for tests
impl Default for SignatureIdentifier {
    fn default() -> Self {
        Self {
            derivator: SignatureDerivator::Ed25519Sha512,
            signature: vec![0; 64],
        }
    }
}

impl Derivable for SignatureIdentifier {
    fn derivative(&self) -> Vec<u8> {
        self.signature.to_owned()
    }
    fn derivation_code(&self) -> String {
        self.derivator.to_str()
    }
}

impl Display for SignatureIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str(),)
    }
}

impl FromStr for SignatureIdentifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let code = SignatureDerivator::from_str(s)?;
        if s.len() == code.material_len() {
            Ok(Self::new(
                code,
                &general_purpose::URL_SAFE_NO_PAD
                    .decode(&s[code.code_len()..code.material_len()])?,
            ))
        } else {
            Err(Error::Semantic(format!(
                "Incorrect Prefix Length: {}",
                s.len()
            )))
        }
    }
}

impl Serialize for SignatureIdentifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_str())
    }
}

impl<'de> Deserialize<'de> for SignatureIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<SignatureIdentifier, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s =
            <std::string::String as Deserialize>::deserialize(deserializer)?;

        SignatureIdentifier::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use super::{SignatureDerivator, SignatureIdentifier};

    use crate::identifier::KeyIdentifier;

    #[test]
    fn test_to_from_string() {
        let message = b"message";
        // Ed25519Sha512
        let si_str = "SETCkX4WX_KcMeLKdwrtN8DGD49z7gbPfYV0Ao8C-B_dcKRj-7TXmv\
        LdKNHG27GDOvgCyWzFMMI23rw2sSssHjBQ";
        let si = SignatureIdentifier::from_str(&si_str).unwrap();
        let key_str = "EYS8MQAh_m740mHnjPMm9IgY9RGojzFak6ELaTsQQZx8";
        let ki = KeyIdentifier::from_str(&key_str).unwrap();
        assert!(ki.verify(message, &si).is_ok());
        // ECDSAsecp256k1
        #[cfg(feature = "secp256k1")]
        {
            let sig_str = "SSdKRafkDIPL3IM8zc5RfGcNo502fQxK-3pzOkNCO8tg4tEyOZUwx\
            qntzDmAwaHINVAN7hwHYfVq5HabqEodrxxQ";
            let si = SignatureIdentifier::from_str(&sig_str).unwrap();
            let key_str = "SAsH8KCN4qfIKmas-2HZeI4IRhTbmMlsjC0EunOP66dqy";
            let ki = KeyIdentifier::from_str(&key_str).unwrap();
            assert!(ki.verify(message, &si).is_ok());
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        // Ed25519Sha512
        let sig_str = "SETCkX4WX_KcMeLKdwrtN8DGD49z7gbPfYV0Ao8C-B_dcKRj-7TXmv\
        LdKNHG27GDOvgCyWzFMMI23rw2sSssHjBQ";
        let si = SignatureIdentifier::from_str(sig_str).unwrap();
        let ser_si = serde_json::to_string_pretty(&si).unwrap();
        let des_si: SignatureIdentifier =
            serde_json::from_str(&ser_si).unwrap();
        assert_eq!(si, des_si);
        assert_eq!(si.derivator, SignatureDerivator::Ed25519Sha512);
        // ECDSAsecp256k1
        #[cfg(feature = "secp256k1")]
        {
            let sig_str = "SSRFbutVG3-KHv_Fuexdx24aukwvj_RqN9jiPt9EQyDYRWsMJ-kpcLfX7\
            CHmERmULScNSiG2l4_DDQF1qui8rEjQ";
            let si = SignatureIdentifier::from_str(sig_str).unwrap();
            let ser_si = serde_json::to_string_pretty(&si).unwrap();
            let des_si: SignatureIdentifier =
                serde_json::from_str(&ser_si).unwrap();
            assert_eq!(si, des_si);
            assert_eq!(si.derivator, SignatureDerivator::ECDSAsecp256k1);
        }
    }
}
