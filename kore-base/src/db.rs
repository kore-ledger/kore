// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Store module.
//!

use crate::{DbConfig, Error};

use rocksdb_db::{RocksDbManager, RocksDbStore};
#[cfg(feature = "sqlite")]
use sqlite_db::SqliteManager;
use store::{
    database::{Collection, DbManager},
    store::{PersistentActor, Store},
    Error as StoreError,
};

#[derive(Clone)]
pub enum Database {
    RocksDb(RocksDbManager),
    #[cfg(feature = "sqlite")]
    SQLite(SqliteManager),
}

impl Database {
    pub fn open(config: &DbConfig) -> Result<Self, Error> {
        match config {
            DbConfig::Rocksdb { path } => {
                let manager = RocksDbManager::new(&path);
                Ok(Database::RocksDb(manager))
            }
            #[cfg(feature = "sqlite")]
            DbConfig::SQLite { path } => {
                let manager = SqliteManager::new(&path);
                Ok(Database::SQLite(manager))
            }
            #[allow(unreachable_patterns)]
            _ => Err(Error::Store("Database not supported".to_string())),
        }
    }

    pub async fn create_store<P>(&self, name: &str) -> Result<Store<P>, Error>
    where
        P: PersistentActor,
    {
        Store::<P>::new(name, self.clone())
            .map_err(|e| Error::Store(format!("Failed to create store: {}", e)))
    }
}

impl DbManager<DbCollection> for Database {
    fn create_collection(
        &self,
        name: &str,
    ) -> Result<DbCollection, StoreError> {
        match self {
            Database::RocksDb(manager) => {
                let store = manager.create_collection(name)?;
                Ok(DbCollection::RocksDb(store))
            }
            #[cfg(feature = "sqlite")]
            Database::SQLite(manager) => {
                let store = manager.create_collection(name)?;
                Ok(DbCollection::SQLite(store))
            }
            #[allow(unreachable_patterns)]
            _ => Err(StoreError::CreateStore(
                "Database not supported".to_string(),
            )),
        }
    }
}

pub enum DbCollection {
    RocksDb(RocksDbStore),
    #[cfg(feature = "sqlite")]
    SQLite(sqlite_db::SqliteCollection),
}

impl Collection for DbCollection {
    fn name(&self) -> &str {
        match self {
            DbCollection::RocksDb(store) => store.name(),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.name(),
        }
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, store::Error> {
        match self {
            DbCollection::RocksDb(store) => store.get(key),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.get(key),
        }
    }

    fn put(&mut self, key: &str, data: &[u8]) -> Result<(), store::Error> {
        match self {
            DbCollection::RocksDb(store) => store.put(key, data),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.put(key, data),
        }
    }

    fn del(&mut self, key: &str) -> Result<(), store::Error> {
        match self {
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
            DbCollection::RocksDb(store) => store.iter(reverse),
            #[cfg(feature = "sqlite")]
            DbCollection::SQLite(store) => store.iter(reverse),
        }
    }
}
