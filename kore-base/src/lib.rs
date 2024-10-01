// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later
#![recursion_limit = "256"]
mod api;
mod approval;
pub mod config;
mod db;
mod distribution;
pub mod error;
mod evaluation;
mod governance;
mod helpers;
mod model;
mod node;
mod request;
mod subject;
mod validation;

use actor::{ActorSystem, SystemRef};
pub use api::Api;
use async_std::sync::RwLock;
pub use config::{Config as KoreBaseConfig, DbConfig};
use db::Database;
pub use error::Error;
use governance::json_schema::JsonSchema;
use governance::schema;
pub use governance::{init::init_state, Governance};
use helpers::encrypted_pass::EncryptedPass;
pub use helpers::network::*;
use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
pub use model::event::Event;
pub use model::request::*;
pub use model::signature::*;
pub use model::HashId;
pub use model::ValueWrapper;
pub use node::{Node, NodeMessage, NodeResponse, SubjectsTypes};
pub use subject::{Subject, SubjectCommand, SubjectResponse, SubjectState};
pub use validation::{
    Validation, ValidationCommand, ValidationInfo, ValidationResponse,
};

use lazy_static::lazy_static;

use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    /// The digest derivator for the system.
    pub static ref DIGEST_DERIVATOR: Mutex<DigestDerivator> = Mutex::new(DigestDerivator::Blake3_256);
    /// The key derivator for the system.
    pub static ref KEY_DERIVATOR: Mutex<KeyDerivator> = Mutex::new(KeyDerivator::Ed25519);

    pub static ref SCHEMAS: RwLock<HashMap<String, JsonSchema>> = {
        let mut schemas = HashMap::new();
        if let Ok(json_schema) = JsonSchema::compile(&schema()) {
            schemas.insert("governance".to_owned(), json_schema);
        };

        RwLock::new(schemas)
    };

    pub static ref CONTRACTS: RwLock<HashMap<String, Vec<u8>>> = {
        let contracts = HashMap::new();

        RwLock::new(contracts)
    };
}

pub async fn system(
    config: KoreBaseConfig,
    password: &str,
) -> Result<SystemRef, Error> {
    
    // Update statics.
    if let Ok(mut derivator) = DIGEST_DERIVATOR.lock() {
        *derivator = config.digest_derivator;
    }
    if let Ok(mut derivator) = KEY_DERIVATOR.lock() {
        *derivator = config.key_derivator;
    }

    // Create de actor system.
    let (system, mut runner) = ActorSystem::create();

    // Build database manager.
    let db_manager = Database::open(&config.database)?;
    system.add_helper("store", db_manager).await;

    // Helper memory encryption for passwords to be used in secure stores.
    let encrypted_pass = EncryptedPass::new(password)?;
    system.add_helper("encrypted_pass", encrypted_pass).await;

    // Spawn the runner.
    tokio::spawn(async move {
        runner.run().await;
    });

    Ok(system)
}

#[cfg(test)]
pub mod tests {

    use identity::identifier::derive::KeyDerivator;

    use super::*;

    #[derive(Debug, Clone)]
    pub struct Dummy;

    #[tokio::test]
    async fn test_system() {
        let system = create_system().await;
        let db: Option<Database> = system.get_helper("store").await;
        assert!(db.is_some());
        let ep: Option<EncryptedPass> =
            system.get_helper("encrypted_pass").await;
        assert!(ep.is_some());
        let any: Option<Dummy> = system.get_helper("dummy").await;
        assert!(any.is_none());
    }

    pub async fn create_system() -> SystemRef {
        let dir =
            tempfile::tempdir().expect("Can not create temporal directory.");
        let path = dir.path().to_str().unwrap().to_owned();
        let config = KoreBaseConfig::new(&path);
        system(config, "password").await.unwrap()
    }
}
