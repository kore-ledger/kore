// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::{ActorSystem, SystemRef};
use tokio_util::sync::CancellationToken;

use crate::{
    db::Database,
    external_db::DBManager,
    helpers::{db::ExternalDB, encrypted_pass::EncryptedPass, sink::KoreSink},
    Error, KoreBaseConfig, DIGEST_DERIVATOR, KEY_DERIVATOR,
};

pub async fn system(
    config: KoreBaseConfig,
    password: &str,
    token: Option<CancellationToken>,
) -> Result<SystemRef, Error> {
    // Update statics.
    if let Ok(mut derivator) = DIGEST_DERIVATOR.lock() {
        *derivator = config.digest_derivator;
    }
    if let Ok(mut derivator) = KEY_DERIVATOR.lock() {
        *derivator = config.key_derivator;
    }

    // Create de actor system.
    let (system, mut runner) = ActorSystem::create(token);

    system.add_helper("config", config.clone()).await;

    // Build database manager.
    let db = Database::open(&config.kore_db);
    system.add_helper("store", db).await;

    // Build sink manager.
    let kore_sink = KoreSink::new(config.sink);
    system.add_helper("sink", kore_sink).await;

    // Helper memory encryption for passwords to be used in secure stores.
    let encrypted_pass = EncryptedPass::new(password)?;
    system.add_helper("encrypted_pass", encrypted_pass).await;

    let db_manager = DBManager::new(config.garbage_collector);
    let db_manager_actor = system
        .create_root_actor("db_manager", db_manager)
        .await
        .map_err(|e| Error::System(e.to_string()))?;

    let ext_db =
        ExternalDB::build(config.external_db, db_manager_actor).await?;

    system.add_helper("ext_db", ext_db).await;

    // Spawn the runner.
    tokio::spawn(async move {
        runner.run().await;
    });

    Ok(system)
}

#[cfg(test)]
pub mod tests {

    use crate::config::{ExternalDbConfig, KoreDbConfig};
    use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
    use network::Config as NetworkConfig;
    use serial_test::serial;
    use std::{fs, time::Duration};
    use test_log::test;

    use async_std::sync::RwLock;

    use crate::{
        governance::{json_schema::JsonSchema, schema},
        GOVERNANCE,
    };

    use super::*;

    #[derive(Debug, Clone)]
    pub struct Dummy;

    #[test(tokio::test)]
    #[serial]
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

    pub fn create_temp_dir() -> String {
        let path = temp_dir();

        if fs::metadata(&path).is_err() {
            fs::create_dir_all(&path).unwrap();
        }
        path
    }

    fn temp_dir() -> String {
        let dir =
            tempfile::tempdir().expect("Can not create temporal directory.");
        dir.path().to_str().unwrap().to_owned()
    }

    pub async fn create_system() -> SystemRef {
        let schema = JsonSchema::compile(&schema()).unwrap();

        let _ = GOVERNANCE.get_or_init(|| RwLock::new(schema));

        let dir =
            tempfile::tempdir().expect("Can not create temporal directory.");
        let path = dir.path().to_str().unwrap();

        let newtork_config = NetworkConfig::new(
            network::NodeType::Bootstrap,
            vec![],
            vec![],
            vec![],
            false,
        );
        let config = KoreBaseConfig {
            key_derivator: KeyDerivator::Ed25519,
            digest_derivator: DigestDerivator::Blake3_256,
            kore_db: KoreDbConfig::build(path),
            external_db: ExternalDbConfig::build(&create_temp_dir()),
            network: newtork_config,
            contracts_dir: create_temp_dir(),
            always_accept: false,
            garbage_collector: Duration::from_secs(500),
            sink: "".to_owned(),
        };

        let sys = system(config.clone(), "password", None).await.unwrap();
        sys
    }
}
