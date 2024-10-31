use crate::{error::Error, request::{manager::RequestManagerEvent, RequestHandlerEvent}};

use actor::Subscriber;
use async_trait::async_trait;
#[cfg(feature = "sqlite-local")]
use sqlite::SqliteLocal;
#[cfg(feature = "sqlite-local")]
mod sqlite;


#[async_trait]
pub trait Querys {
    async fn get_request_id_status(&self, request_id: &str) -> String;
}

#[async_trait]
impl Querys for LocalDB {
    async fn get_request_id_status(&self, request_id: &str) -> String {
        match self {
            #[cfg(feature = "sqlite-local")]
            LocalDB::SqliteLocal(sqlite_local) => sqlite_local.get_request_id_status(request_id).await,
        }
    }
}

#[derive(Clone)]
pub enum LocalDB {
    #[cfg(feature = "sqlite-local")]
    SqliteLocal(SqliteLocal)
}

impl LocalDB {
    #[cfg(feature = "sqlite-local")]
    pub async fn sqlite(path: &str) -> Result<Self, Error> {

        let sqlite = SqliteLocal::new(path).await?;
        Ok(LocalDB::SqliteLocal(sqlite))
    }

    pub fn get_request_manager(&self) -> impl Subscriber<RequestManagerEvent> {
        match self {
            #[cfg(feature = "sqlite-local")]
            LocalDB::SqliteLocal(sqlite_local) => sqlite_local.clone(),
        }
    }

    pub fn get_request_handler(&self) -> impl Subscriber<RequestHandlerEvent> {
        match self {
            #[cfg(feature = "sqlite-local")]
            LocalDB::SqliteLocal(sqlite_local) => sqlite_local.clone(),
        }
    }
}