use actor::{ActorSystem, SystemRef};

use crate::{
    db::Database, helpers::encrypted_pass::EncryptedPass, Error,
    KoreBaseConfig, DIGEST_DERIVATOR, KEY_DERIVATOR,
};

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
    let db_manager = Database::open(&config.kore_db)?;
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

    use std::{fs, time::Duration};

    use async_std::sync::RwLock;

    use crate::{
        governance::{json_schema::JsonSchema, schema},
        helpers::db::LocalDB,
        local_db::DBManager,
        GOVERNANCE,
    };

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

        let config = KoreBaseConfig::new(path, "");

        let sys = system(config, "password").await.unwrap();

        let db_manager = DBManager::new(Duration::from_secs(5));
        let db_manager_actor = sys
            .create_root_actor("db_manager", db_manager)
            .await
            .unwrap();

        let local_db = LocalDB::sqlite(
            &format!("{}/database.db", create_temp_dir()),
            db_manager_actor,
        )
        .await
        .unwrap();
        sys.add_helper("local_db", local_db).await;

        sys
    }
}
