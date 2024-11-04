use crate::{error::Error, local_db::DBManager, request::{manager::RequestManagerEvent, RequestHandlerEvent}};

use actor::{ActorRef, Subscriber};
use async_trait::async_trait;
#[cfg(feature = "sqlite-local")]
use sqlite::SqliteLocal;
#[cfg(feature = "sqlite-local")]
mod sqlite;

#[async_trait]
pub trait Querys {
    async fn get_request_id_status(&self, request_id: &str) -> Result<String, Error>;
    async fn del_request(&self, request_id: &str) -> Result<(), Error>;
}

#[derive(Clone)]
pub enum LocalDB {
    #[cfg(feature = "sqlite-local")]
    SqliteLocal(SqliteLocal)
}


impl LocalDB {
    #[cfg(feature = "sqlite-local")]
    pub async fn sqlite(path: &str, manager: ActorRef<DBManager>) -> Result<Self, Error> {

        let sqlite = SqliteLocal::new(path, manager).await?;
        Ok(LocalDB::SqliteLocal(sqlite))
    }

    pub async fn get_request_id_status(&self, request_id: &str) -> String {
        let result = match self {
            #[cfg(feature = "sqlite-local")]
            LocalDB::SqliteLocal(sqlite_local) => sqlite_local.get_request_id_status(request_id).await,
        };

        match result {
            Ok(id) => id,
            Err(e) => { todo!() }
        }
    }
    pub async fn del_request(&self, request_id: &str) {
        if let Err(e) = match self {
            #[cfg(feature = "sqlite-local")]
            LocalDB::SqliteLocal(sqlite_local) => sqlite_local.del_request(request_id).await,
        } {
            todo!()
        };
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