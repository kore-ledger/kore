// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later
#![recursion_limit = "256"]

mod api;
pub mod config;
mod db;
mod error;
mod governance;
mod helpers;
mod model;
mod node;
mod request;
mod subject;

pub use api::Api;
pub use config::{Config, DbConfig};
pub use error::Error;

use node::Node;
use db::Database;
use actor::{ActorSystem, SystemRef};
use identity::keys::KeyPair;
use helpers::encrypted_pass::EncryptedPass;


pub async fn system(config: Config, password: &str) -> Result<SystemRef, Error> {
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

    use super::*;

    #[derive(Debug, Clone)]
    pub struct Dummy;

    #[tokio::test]
    async fn test_system() {
        let dir =
            tempfile::tempdir().expect("Can not create temporal directory.");
        let path = dir.path().to_str().unwrap().to_owned(); 
        let db = DbConfig::Rocksdb { path };
        let config = Config { database: db};
        let system = system(config, "password").await.unwrap();
        let db: Option<Database> = system.get_helper("store").await;
        assert!(db.is_some());
        let ep: Option<EncryptedPass> = system.get_helper("encrypted_pass").await;
        assert!(ep.is_some());
        let any: Option<Dummy> = system.get_helper("dummy").await;
        assert!(any.is_none());

    }
} 
