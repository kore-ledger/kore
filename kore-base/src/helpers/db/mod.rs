// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    approval::approver::ApproverEvent,
    error::Error,
    external_db::DBManager,
    model::event::Ledger,
    request::{manager::RequestManagerEvent, RequestHandlerEvent},
    subject::{event::LedgerEventEvent, sinkdata::SinkDataEvent},
    Signed,
};

use crate::config::ExternalDbConfig;

use actor::{ActorRef, Subscriber};
use async_std::fs;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "ext-sqlite")]
use sqlite::SqliteLocal;
use std::path::Path;
#[cfg(feature = "ext-sqlite")]
mod sqlite;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignaturesDB {
    pub subject_id: String,
    pub sn: u64,
    pub signatures_eval: String,
    pub signatures_appr: String,
    pub signatures_vali: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubjectDB {
    pub subject_id: String,
    pub governance_id: String,
    pub genesis_gov_version: u64,
    pub namespace: String,
    pub schema_id: String,
    pub owner: String,
    pub creator: String,
    pub active: String,
    pub sn: u64,
    pub properties: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventDB {
    pub subject_id: String,
    pub sn: u64,
    pub data: String,
    pub event_req: String,
    pub succes: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paginator {
    pub pages: u64,
    pub next: Option<u64>,
    pub prev: Option<u64>,
}

#[async_trait]
pub trait Querys {
    // request
    async fn get_request_id_status(
        &self,
        request_id: &str,
    ) -> Result<String, Error>;
    async fn del_request(&self, request_id: &str) -> Result<(), Error>;
    // approver
    async fn get_approve_req(&self, subject_id: &str) -> Result<Value, Error>;
    // validators (not for user use).
    async fn get_last_validators(
        &self,
        subject_id: &str,
    ) -> Result<String, Error>;
    // events
    async fn get_events(
        &self,
        subject_id: &str,
        quantity: Option<u64>,
        page: Option<u64>,
    ) -> Result<Value, Error>;
    
    // events sn
    async fn get_events_sn(
        &self,
        subject_id: &str,
        sn: u64,
    ) -> Result<Value, Error>;

    // n first or last events
    async fn get_first_or_end_events(
        &self,
        subject_id: &str,
        quantity: u64,
        reverse: bool,
        sucess: Option<bool>,
    ) -> Result<Value, Error>;

    // subject
    async fn get_subject_state(&self, subject_id: &str)
        -> Result<Value, Error>;

    // signatures
    async fn get_signatures(&self, subject_id: &str) -> Result<Value, Error>;
}

#[derive(Clone)]
pub enum ExternalDB {
    #[cfg(feature = "ext-sqlite")]
    SqliteLocal(SqliteLocal),
}

impl ExternalDB {
    pub async fn build(
        ext_db: ExternalDbConfig,
        manager: ActorRef<DBManager>,
    ) -> Result<Self, Error> {
        match ext_db {
            #[cfg(feature = "ext-sqlite")]
            ExternalDbConfig::SQLite { path } => {
                if !Path::new(&path).exists() {
                    fs::create_dir_all(&path).await.map_err(|e| {
                        Error::Node(format!("Can not create src dir: {}", e))
                    })?;
                }
                let path = format!("{}/database.db", path);
                let sqlite = SqliteLocal::new(&path, manager).await?;
                Ok(ExternalDB::SqliteLocal(sqlite))
            }
        }
    }

    pub fn get_request_manager(&self) -> impl Subscriber<RequestManagerEvent> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => sqlite_local.clone(),
        }
    }

    pub fn get_request_handler(&self) -> impl Subscriber<RequestHandlerEvent> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => sqlite_local.clone(),
        }
    }

    pub fn get_approver(&self) -> impl Subscriber<ApproverEvent> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => sqlite_local.clone(),
        }
    }

    pub fn get_ledger_event(&self) -> impl Subscriber<LedgerEventEvent> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => sqlite_local.clone(),
        }
    }

    pub fn get_subject(&self) -> impl Subscriber<Signed<Ledger>> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => sqlite_local.clone(),
        }
    }

    pub fn get_sink_data(&self) -> impl Subscriber<SinkDataEvent> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => sqlite_local.clone(),
        }
    }
}

#[async_trait]
impl Querys for ExternalDB {
    async fn get_signatures(&self, subject_id: &str) -> Result<Value, Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local.get_signatures(subject_id).await
            }
        }
    }

    async fn get_subject_state(
        &self,
        subject_id: &str,
    ) -> Result<Value, Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local.get_subject_state(subject_id).await
            }
        }
    }

    async fn get_events(
        &self,
        subject_id: &str,
        quantity: Option<u64>,
        page: Option<u64>,
    ) -> Result<Value, Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local.get_events(subject_id, quantity, page).await
            }
        }
    }

    async fn get_events_sn(
        &self,
        subject_id: &str,
        sn: u64,
    ) -> Result<Value, Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local.get_events_sn(subject_id, sn).await
            }
        }
    }

    async fn get_first_or_end_events(
        &self,
        subject_id: &str,
        quantity: u64,
        reverse: bool,
        sucess: Option<bool>,
    ) -> Result<Value, Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local
                    .get_first_or_end_events(subject_id, quantity, reverse, sucess)
                    .await
            }
        }
    }

    async fn get_request_id_status(
        &self,
        request_id: &str,
    ) -> Result<String, Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local.get_request_id_status(request_id).await
            }
        }
    }

    async fn del_request(&self, request_id: &str) -> Result<(), Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local.del_request(request_id).await
            }
        }
    }

    async fn get_approve_req(&self, subject_id: &str) -> Result<Value, Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local.get_approve_req(subject_id).await
            }
        }
    }

    async fn get_last_validators(
        &self,
        subject_id: &str,
    ) -> Result<String, Error> {
        match self {
            #[cfg(feature = "ext-sqlite")]
            ExternalDB::SqliteLocal(sqlite_local) => {
                sqlite_local.get_last_validators(subject_id).await
            }
        }
    }
}
