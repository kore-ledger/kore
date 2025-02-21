// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Store module.
//!

use crate::{config::KoreDbConfig, helpers::encrypted_pass::EncryptedPass};

use actor::{ActorContext, Error as ActorError};
#[cfg(feature = "rocksdb")]
use rocksdb_db::{RocksDbManager, RocksDbStore};
#[cfg(feature = "sqlite")]
use sqlite_db::SqliteManager;
use store::{
    database::{Collection, DbManager},
    store::PersistentActor,
    Error as StoreError,
};

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
                let manager = SqliteManager::new(&path)?;
                Ok(Database::SQLite(manager))
            }
        }
    }
}

impl DbManager<DbCollection> for Database {
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
}

pub enum DbCollection {
    #[cfg(feature = "rocksdb")]
    RocksDb(RocksDbStore),
    #[cfg(feature = "sqlite")]
    SQLite(sqlite_db::SqliteCollection),
}

impl Collection for DbCollection {
    fn name(&self) -> &str {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => store.name(),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.name(),
        }
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, store::Error> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => store.get(key),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.get(key),
        }
    }

    fn put(&mut self, key: &str, data: &[u8]) -> Result<(), store::Error> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => store.put(key, data),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.put(key, data),
        }
    }

    fn del(&mut self, key: &str) -> Result<(), store::Error> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => store.del(key),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.del(key),
        }
    }

    fn iter<'a>(
        &'a self,
        reverse: bool,
    ) -> Box<dyn Iterator<Item = (String, Vec<u8>)> + 'a> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => store.iter(reverse),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.iter(reverse),
        }
    }

    fn purge(&mut self) -> Result<(), StoreError> {
        match self {
            #[cfg(feature = "rocksdb")]
            DbCollection::RocksDb(store) => store.purge(),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.purge(),
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
        let key = if encrypt {
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
