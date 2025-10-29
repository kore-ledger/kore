//! # Store module.
//!

use crate::{config::KoreDbConfig, helpers::encrypted_pass::EncryptedPass};

#[cfg(feature = "sqlite")]
use rush::SqliteManager;
use rush::{ActorContext, ActorError};
use rush::{Collection, DbManager, PersistentActor, State, StoreError};
#[cfg(feature = "rocksdb")]
use rush::{RocksDbManager, RocksDbStore};

use async_trait::async_trait;

#[derive(Clone)]
pub enum Database {
    #[cfg(feature = "rocksdb")]
    RocksDb(RocksDbManager),
    #[cfg(feature = "sqlite")]
    SQLite(SqliteManager),
}

impl Database {
    pub fn open(config: &KoreDbConfig) -> Result<Self, StoreError> {
        match config {
            #[cfg(feature = "rocksdb")]
            KoreDbConfig::Rocksdb { path } => {
                let manager = RocksDbManager::new(path)?;
                Ok(Database::RocksDb(manager))
            }
            #[cfg(feature = "sqlite")]
            KoreDbConfig::Sqlite { path } => {
                let manager = SqliteManager::new(path)?;
                Ok(Database::SQLite(manager))
            }
        }
    }
}

impl DbManager<DbCollection, DbCollection> for Database {
    fn create_collection(
        &self,
        name: &str,
        prefix: &str,
    ) -> Result<DbCollection, StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            Database::RocksDb(manager) => {
                let store = manager.create_collection(name, prefix)?;
                Ok(DbCollection::RocksDb(store))
            }
            #[cfg(feature = "sqlite")]
            Database::SQLite(manager) => {
                let store = manager.create_collection(name, prefix)?;
                Ok(DbCollection::SQLite(store))
            }
        }
    }

    fn create_state(
        &self,
        name: &str,
        prefix: &str,
    ) -> Result<DbCollection, StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            Database::RocksDb(manager) => {
                let store = manager.create_state(name, prefix)?;
                Ok(DbCollection::RocksDb(store))
            }
            #[cfg(feature = "sqlite")]
            Database::SQLite(manager) => {
                let store = manager.create_state(name, prefix)?;
                Ok(DbCollection::SQLite(store))
            }
        }
    }
}

pub enum DbCollection {
    #[cfg(feature = "rocksdb")]
    RocksDb(RocksDbStore),
    #[cfg(feature = "sqlite")]
    SQLite(rush::SqliteCollection),
}

impl Collection for DbCollection {
    fn name(&self) -> &str {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => Collection::name(store),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => Collection::name(store),
        }
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => Collection::get(store, key),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => Collection::get(store, key),
        }
    }

    fn put(&mut self, key: &str, data: &[u8]) -> Result<(), StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => Collection::put(store, key, data),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => Collection::put(store, key, data),
        }
    }

    fn del(&mut self, key: &str) -> Result<(), StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => Collection::del(store, key),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => Collection::del(store, key),
        }
    }

    fn iter<'a>(
        &'a self,
        reverse: bool,
    ) -> Box<dyn Iterator<Item = (String, Vec<u8>)> + 'a> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => Collection::iter(store, reverse),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => Collection::iter(store, reverse),
        }
    }

    fn purge(&mut self) -> Result<(), StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => Collection::purge(store),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => Collection::purge(store),
        }
    }
}

impl State for DbCollection {
    fn name(&self) -> &str {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => State::name(store),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => State::name(store),
        }
    }

    fn get(&self) -> Result<Vec<u8>, StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => State::get(store),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => State::get(store),
        }
    }

    fn put(&mut self, data: &[u8]) -> Result<(), StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => State::put(store, data),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => State::put(store, data),
        }
    }

    fn del(&mut self) -> Result<(), StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => State::del(store),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => State::del(store),
        }
    }

    fn purge(&mut self) -> Result<(), StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => State::purge(store),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => State::purge(store),
        }
    }
}

#[async_trait]
pub trait Storable: PersistentActor {
    async fn init_store(
        &mut self,
        name: &str,
        prefix: Option<String>,
        encrypt: bool,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        // Gets database
        let db = match ctx.system().get_helper::<Database>("store").await {
            Some(db) => db,
            None => {
                return Err(ActorError::CreateStore(
                    "Database not found".to_string(),
                ));
            }
        };
        // Encrypted store?
        let key: Option<[u8; 32]> = if encrypt {
            if let Some(enc) = ctx
                .system()
                .get_helper::<EncryptedPass>("encrypted_pass")
                .await
            {
                enc.key()
            } else {
                None
            }
        } else {
            None
        };
        // Start store
        self.start_store(name, prefix, ctx, db, key).await?;
        Ok(())
    }
}
