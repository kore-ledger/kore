// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later
#![recursion_limit = "256"]

mod api;
mod approval;
pub mod config;
mod db;
mod error;
mod evaluation;
mod governance;
mod helpers;
mod model;
mod node;
mod request;
mod subject;
mod validation;

pub use api::Api;
pub use config::{Config, DbConfig};
pub use error::Error;

use actor::{ActorSystem, SystemRef};
use db::Database;
use helpers::encrypted_pass::EncryptedPass;
use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};

use lazy_static::lazy_static;

use std::sync::Mutex;

lazy_static! {
    /// The digest derivator for the system.
    pub static ref DIGEST_DERIVATOR: Mutex<DigestDerivator> = Mutex::new(DigestDerivator::Blake3_256);
    /// The key derivator for the system.
    pub static ref KEY_DERIVATOR: Mutex<KeyDerivator> = Mutex::new(KeyDerivator::Ed25519);
}

pub async fn system(
    config: Config,
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
        let config = Config::new(&path);
        system(config, "password").await.unwrap()
    }
}
