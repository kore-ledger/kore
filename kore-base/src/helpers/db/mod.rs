use crate::{
    approval::approver::{ApprovalState, ApprovalStateRes, ApproverEvent},
    error::Error,
    external_db::DBManager,
    model::event::Ledger,
    request::{manager::RequestManagerEvent, RequestHandlerEvent},
    subject::{event::LedgerEventEvent, sinkdata::SinkDataEvent},
    Signed,
};

use crate::config::ExternalDbConfig;

use actor::{ActorRef, Subscriber};
use async_trait::async_trait;
#[cfg(feature = "ext-sqlite")]
use sqlite::SqliteLocal;
#[cfg(feature = "ext-sqlite")]
mod sqlite;

#[async_trait]
pub trait Querys {
    // request
    async fn get_request_id_status(
        &self,
        request_id: &str,
    ) -> Result<String, Error>;
    async fn del_request(&self, request_id: &str) -> Result<(), Error>;
    // approver
    async fn get_approve_req(
        &self,
        subject_id: &str,
    ) -> Result<(String, String), Error>;
    // validators
    async fn get_last_validators(
        &self,
        subject_id: &str,
    ) -> Result<String, Error>;
}

#[derive(Clone)]
pub enum ExternalDB {
    #[cfg(feature = "ext-sqlite")]
    SqliteLocal(SqliteLocal),
}

impl ExternalDB {
    #[cfg(feature = "ext-sqlite")]
    pub async fn build(
        ext_db: ExternalDbConfig,
        manager: ActorRef<DBManager>,
    ) -> Result<Self, Error> {
        match ext_db {
            #[cfg(feature = "ext-sqlite")]
            ExternalDbConfig::SQLite { path } => {
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

    async fn get_approve_req(
        &self,
        subject_id: &str,
    ) -> Result<(String, String), Error> {
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
