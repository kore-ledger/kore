use std::sync::{Arc, RwLock};

use actor::Subscriber;
use async_std::sync::Mutex;
use async_trait::async_trait;
use tokio_rusqlite::{params, Connection, OpenFlags, Result as SqliteError};

use crate::error::Error;
use crate::model::event;
use crate::request::manager::RequestManagerEvent;
use crate::request::state::RequestManagerState;
use crate::request::RequestHandlerEvent;

use super::Querys;

#[derive(Clone)]
pub struct SqliteLocal {
    conn: Connection,
}

#[async_trait]
impl Querys for SqliteLocal {
    async fn get_request_id_status(&self, request_id: &str) -> String {

        let request_id = request_id.to_owned();
        let state: String = match self
        .conn
        .call(move |conn| {
            let sql = "SELECT state FROM request WHERE id = ?1";

            match conn.query_row(&sql, params![request_id], |row| row.get(0)) {
                Ok(result) => Ok(result),
                Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e))
            }
        })
        .await {
            Ok(state) => state,
            Err(e) => todo!()
        };

        state
    }
}

impl SqliteLocal {
    pub async fn new(path: &str) -> Result<Self, Error> {
        let flags = OpenFlags::default();
        let conn =
            Connection::open_with_flags(path, flags)
                .await
                .map_err(|e| {
                    Error::Database(format!(
                        "SQLite fail open connection: {}",
                        e
                    ))
                })?;

        conn.call(|conn| {
            let sql = "CREATE TABLE IF NOT EXISTS request (id TEXT NOT NULL, state TEXT NOT NULL, PRIMARY KEY (id, state))";
            let _ = conn.execute(sql, ())?;
            Ok(())
        }).await.map_err(|e| Error::Database(format!("Can not create request table: {}",e)))?;

        Ok(SqliteLocal { conn })
    }
}

// TODO Actor nuevo para que si falla la escritura en la base de datos
// el nodo sea capaz de manejar la situación, ya que sino no habría
// comunicación
#[async_trait]
impl Subscriber<RequestManagerEvent> for SqliteLocal {
    async fn notify(&self, event: RequestManagerEvent) {
        let state = match event.state {
            RequestManagerState::Starting => return,
            RequestManagerState::Evaluation => "In Evaluation".to_owned(),
            RequestManagerState::Approval { .. } => "In Approval".to_owned(),
            RequestManagerState::Validation(..) => "In Validation".to_owned(),
            RequestManagerState::Distribution { .. } => {
                "In Distribution".to_owned()
            }
        };
        if let Err(e) = self
            .conn
            .call(move |conn| {
                let sql = "UPDATE request SET state = ?1 WHERE id = ?2";
                let _ = conn.execute(sql, params![state, event.id])?;

                Ok(())
            })
            .await
            .map_err(|e| {
                Error::Database(format!("Can not create request table: {}", e))
            })
        {
            todo!()
        };
    }
}

#[async_trait]
impl Subscriber<RequestHandlerEvent> for SqliteLocal {
    async fn notify(&self, event: RequestHandlerEvent) {
        let (id, state, insert) = match event {
            RequestHandlerEvent::EventToQueue { id, .. } => {
                (id, "In queue".to_owned(), true)
            }
            RequestHandlerEvent::Invalid { id, .. } => {
                (id, "Invalid".to_owned(), false)
            }
            RequestHandlerEvent::FinishHandling { id, .. } => {
                (id, "Finish".to_owned(), false)
            }
            RequestHandlerEvent::EventToHandling { request_id, .. } => {
                (request_id, "In Handling".to_owned(), false)
            }
        };

        let sql = if insert {
            "INSERT INTO request (id, state) VALUES (?1, ?2)".to_owned()
        } else {
            "UPDATE request SET state = ?2 WHERE id = ?1".to_owned()
        };

        let id_clone = id.clone();
        let state_clone = state.clone();
        if let Err(e) = self
            .conn
            .call(move |conn| {
                let _ = conn.execute(&sql, params![id, state])?;

                Ok(())
            })
            .await
            .map_err(|e| {
                Error::Database(format!(
                    "Update request_id {} state {}: {}",
                    id_clone, state_clone, e
                ))
            })
        {
            println!("{}", e);
            todo!()
        };
    }
}
