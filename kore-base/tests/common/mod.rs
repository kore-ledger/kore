use actor::SystemRef;
use identity::{identifier::{derive::{digest::DigestDerivator, KeyDerivator}, DigestIdentifier, KeyIdentifier}, keys::{Ed25519KeyPair, KeyGenerator, KeyMaterial, KeyPair}};
use kore_base::{system, Config, EventRequest, Signature, Signed, StartRequest, ValueWrapper};

pub async fn create_system() -> SystemRef {
    let dir = tempfile::tempdir().expect("Can not create temporal directory.");
    let path = dir.path().to_str().unwrap().to_owned();
    let config = Config::new(&path);
    system(config, "password").await.unwrap()
}

// Create governance request mock.
pub fn create_start_request_mock( key_pair: KeyPair, key_identifier: KeyIdentifier) -> Signed<EventRequest> {
    let req = StartRequest {
        governance_id: DigestIdentifier::default(),
        schema_id: "governance".to_string(),
        namespace: "namespace".to_string(),
        name: "name".to_string(),
        public_key: key_identifier.clone(),
    };
    let content = EventRequest::Create(req);
    let signature =
        Signature::new(&content, &key_pair, DigestDerivator::SHA2_256).unwrap();
    Signed { content, signature }
}

// Mokcs
#[allow(dead_code)]
pub fn issuer_identity(name: &str) -> (KeyPair, KeyIdentifier) {
    let filler = [0u8; 32];
    let mut value = name.as_bytes().to_vec();
    value.extend(filler.iter());
    value.truncate(32);
    let kp = Ed25519KeyPair::from_secret_key(&value);
    let id = KeyIdentifier::new(KeyDerivator::Ed25519, &kp.public_key_bytes());
    (KeyPair::Ed25519(kp), id)
}