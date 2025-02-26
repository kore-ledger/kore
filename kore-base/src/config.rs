// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Configuration module

use std::time::Duration;

use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
use network::Config as NetworkConfig;
use serde::{Deserialize, Deserializer};

/// Node configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    /// Key derivator.
    pub key_derivator: KeyDerivator,
    /// Digest derivator.
    pub digest_derivator: DigestDerivator,
    /// Database configuration.
    pub kore_db: KoreDbConfig,
    /// External database configuration.
    pub external_db: ExternalDbConfig,
    /// Network configuration.
    pub network: NetworkConfig,
    /// Contract dir.
    pub contracts_dir: String,
    /// Approval mode.
    pub always_accept: bool,
    /// Garbage collector acts
    pub garbage_collector: Duration,
    pub sink: String,
}

impl Config {
    pub fn add_path(&mut self, path: &str) {
        self.kore_db.add_path(path);
        self.external_db.add_path(path);

        self.contracts_dir = format!("{}/{}", path, self.contracts_dir);
    }
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum KoreDbConfig {
    /// Rocksdb database.
    #[cfg(feature = "rocksdb")]
    Rocksdb {
        /// Path to the database.
        path: String,
    },
    /// Sqlite database.
    #[cfg(feature = "sqlite")]
    Sqlite {
        /// Path to the database.
        path: String,
    },
}

impl Default for KoreDbConfig {
    fn default() -> Self {
        #[cfg(feature = "rocksdb")]
        return KoreDbConfig::Rocksdb {
            path: "db/local/rockdb".to_owned(),
        };
        #[cfg(feature = "sqlite")]
        return KoreDbConfig::Sqlite {
            path: "db/local/sqlite".to_owned(),
        };
    }
}

impl KoreDbConfig {
        pub fn add_path(&mut self, new_path: &str) {
            match self {
                #[cfg(feature = "rocksdb")]
                KoreDbConfig::Rocksdb { path } => {
                    *path = format!("{}/{}", new_path, path);
                },
                #[cfg(feature = "sqlite")]
                KoreDbConfig::Sqlite { path } => {
                    *path = format!("{}/{}", new_path, path);
                },
            };
        }

        pub fn to_string(&self) -> String {
            match self {
                #[cfg(feature = "rocksdb")]
                KoreDbConfig::Rocksdb { .. } => "Rocksdb",
                #[cfg(feature = "sqlite")]
                KoreDbConfig::Sqlite { .. } => "Sqlite",
            }
            .into()
        }
    

    pub fn build(path: &str) -> Self {
        #[cfg(feature = "rocksdb")]
        return KoreDbConfig::Rocksdb {
            path: path.to_owned(),
        };
        #[cfg(feature = "sqlite")]
        return KoreDbConfig::Sqlite {
            path: path.to_owned(),
        };
    }

    pub fn deserialize_db<'de, D>(
        deserializer: D,
    ) -> Result<KoreDbConfig, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path: String = String::deserialize(deserializer)?;
        #[cfg(feature = "rocksdb")]
        return Ok(KoreDbConfig::Rocksdb { path });
        #[cfg(feature = "sqlite")]
        return Ok(KoreDbConfig::Sqlite { path });
    }
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub enum ExternalDbConfig {
    /// Sqlite database.
    #[cfg(feature = "ext-sqlite")]
    Sqlite {
        /// Path to the database.
        path: String,
    },
}

impl Default for ExternalDbConfig {
    fn default() -> Self {
        #[cfg(feature = "ext-sqlite")]
        return ExternalDbConfig::Sqlite {
            path: "db/ext/ext-sqlite".to_owned(),
        };
    }
}

impl ExternalDbConfig {
    pub fn add_path(&mut self, new_path: &str) {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDbConfig::Sqlite { path } => {
                *path = format!("{}/{}", new_path, path);
            },
        };
    }

    pub fn to_string(&self) -> String {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDbConfig::Sqlite { .. } => "Sqlite",
        }
        .into()
    }

    pub fn build(path: &str) -> Self {
        #[cfg(feature = "ext-sqlite")]
        return ExternalDbConfig::Sqlite {
            path: path.to_owned(),
        };
    }

    pub fn deserialize_db<'de, D>(
        deserializer: D,
    ) -> Result<ExternalDbConfig, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path: String = String::deserialize(deserializer)?;
        #[cfg(feature = "ext-sqlite")]
        return Ok(ExternalDbConfig::Sqlite { path });
    }
}
