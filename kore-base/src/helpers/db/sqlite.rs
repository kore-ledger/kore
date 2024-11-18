use actor::{ActorRef, Subscriber};
use async_trait::async_trait;
use tokio_rusqlite::{params, Connection, OpenFlags, Result as SqliteError};

use crate::approval::approver::ApproverEvent;
use crate::error::Error;
use crate::external_db::{DBManager, DBManagerMessage, DeleteTypes};
use crate::model::event::{Ledger, LedgerValue};
use crate::request::manager::RequestManagerEvent;
use crate::request::state::RequestManagerState;
use crate::request::RequestHandlerEvent;
use crate::subject::event::LedgerEventEvent;
use crate::subject::sinkdata::SinkDataEvent;
use crate::{EventRequest, Signed};

use super::Querys;

#[derive(Clone)]
pub struct SqliteLocal {
    manager: ActorRef<DBManager>,
    conn: Connection,
}

#[async_trait]
impl Querys for SqliteLocal {
    async fn get_request_id_status(
        &self,
        request_id: &str,
    ) -> Result<String, Error> {
        let request_id = request_id.to_owned();
        let state: String = match self
            .conn
            .call(move |conn| {
                let sql = "SELECT state FROM request WHERE id = ?1";

                match conn
                    .query_row(&sql, params![request_id], |row| row.get(0))
                {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(state) => state,
            Err(e) => todo!(),
        };

        Ok(state)
    }

    async fn del_request(&self, request_id: &str) -> Result<(), Error> {
        let request_id = request_id.to_owned();

        if let Err(e) = self
            .conn
            .call(move |conn| {
                let sql = "DELETE FROM request WHERE id = ?1";
                let _ = conn.execute(sql, params![request_id])?;
                Ok(())
            })
            .await
        {
            todo!()
        };

        Ok(())
    }

    async fn get_approve_req(
        &self,
        subject_id: &str,
    ) -> Result<(String, String), Error> {
        let subject_id = subject_id.to_owned();
        let state = match self
            .conn
            .call(move |conn| {
                let sql =
                    "SELECT data, state FROM approval WHERE subject_id = ?1";

                match conn.query_row(&sql, params![subject_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                }) {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(state) => state,
            Err(e) => {
                println!("{}", e);
                todo!()
            }
        };

        Ok(state)
    }

    async fn get_last_validators(
        &self,
        subject_id: &str,
    ) -> Result<String, Error> {
        let subject_id = subject_id.to_owned();
        let validators: String = match self
            .conn
            .call(move |conn| {
                let sql =
                    "SELECT validators FROM validations WHERE subject_id = ?1";

                match conn
                    .query_row(&sql, params![subject_id], |row| row.get(0))
                {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(validators) => validators,
            Err(e) => todo!(),
        };

        Ok(validators)
    }
}

impl SqliteLocal {
    pub async fn new(
        path: &str,
        manager: ActorRef<DBManager>,
    ) -> Result<Self, Error> {
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
            let sql = "CREATE TABLE IF NOT EXISTS request (id TEXT NOT NULL, state TEXT NOT NULL, PRIMARY KEY (id))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS approval (subject_id TEXT NOT NULL, data TEXT NOT NULL, state TEXT NOT NULL, PRIMARY KEY (subject_id))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS validations (subject_id TEXT NOT NULL, validators TEXT NOT NULL, PRIMARY KEY (subject_id))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS events (subject_id TEXT NOT NULL, sn INTEGER NOT NULL, data TEXT NOT NULL, succes TEXT NOT NULL, PRIMARY KEY (subject_id, sn))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS subjects (subject_id TEXT NOT NULL, governance_id TEXT NOT NULL, genesis_gov_version INTEGER NOT NULL, namespace TEXT NOT NULL, schema_id TEXT NOT NULL, owner TEXT NOT NULL, creator TEXT NOT NULL, active TEXT NOT NULL, sn INTEGER NOT NULL, properties TEXT NOT NULL, PRIMARY KEY (subject_id))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS signatures (subject_id TEXT NOT NULL, sn INTEGER NOT NULL, signatures_eval TEXT NOT NULL, signatures_appr TEXT NOT NULL, signatures_vali TEXT NOT NULL, PRIMARY KEY (subject_id))";
            let _ = conn.execute(sql, ())?;
            Ok(())
        }).await.map_err(|e| Error::Database(format!("Can not create request table: {}",e)))?;

        Ok(SqliteLocal { conn, manager })
    }
}

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
                let _ = conn.execute(&sql, params![id_clone, state_clone])?;

                Ok(())
            })
            .await
            .map_err(|e| {
                Error::Database(format!(
                    "Update request_id {} state {}: {}",
                    id, state, e
                ))
            })
        {
            println!("{}", e);
            todo!()
        };

        if state == "Finish" {
            if let Err(e) = self
                .manager
                .tell(DBManagerMessage::Delete(DeleteTypes::Request { id }))
                .await
            {
                todo!()
            }
        }
    }
}

#[async_trait]
impl Subscriber<ApproverEvent> for SqliteLocal {
    async fn notify(&self, event: ApproverEvent) {
        match event {
            ApproverEvent::ChangeState { subject_id, state } => {
                let response = state.to_string();

                if let Err(e) = self
                    .conn
                    .call(move |conn| {
                        let sql =
                            "UPDATE approval SET state = ?1 WHERE subject_id = ?2";
                        let _ =
                            conn.execute(sql, params![response, subject_id])?;

                        Ok(())
                    })
                    .await
                    .map_err(|e| Error::Database(format!(": {}", e)))
                {
                    todo!()
                };
            }
            ApproverEvent::SafeState {
                subject_id,
                request,
                state,
                ..
            } => {
                let request_text = format!("{:?}", request);

                if let Err(e) = self
                    .conn
                    .call(move |conn| {
                        let _ =
                            conn.execute("INSERT OR REPLACE INTO approval (subject_id, data, state) VALUES (?1, ?2, ?3)", params![subject_id, request_text, state.to_string()])?;

                        Ok(())
                    })
                    .await
                    .map_err(|e| {
                        Error::Database(format!(": {}", e))
                    })
                {
                    println!("{}", e);
                    todo!()
                };
            }
        }
    }
}

#[async_trait]
impl Subscriber<LedgerEventEvent> for SqliteLocal {
    async fn notify(&self, event: LedgerEventEvent) {
        let event = match event {
            LedgerEventEvent::WithVal { validators, event } => {
                let subject_id = event.content.subject_id.to_string();
                let Ok(validators) = serde_json::to_string(&validators) else {
                    todo!()
                };

                if let Err(e) = self
                    .conn
                    .call(move |conn| {
                        let _ =
                            conn.execute("INSERT OR REPLACE INTO validations (subject_id, validators) VALUES (?1, ?2)", params![subject_id, validators])?;

                        Ok(())
                    })
                    .await
                    .map_err(|e| {
                        Error::Database(format!(": {}", e))
                    })
                {
                    println!("{}", e);
                    todo!()
                };

                event
            }
            LedgerEventEvent::WithOutVal { event } => event,
        };
        let sn = event.content.sn;
        let subject_id = event.content.subject_id.to_string();
        let Ok(sig_eval) = serde_json::to_string(&event.content.evaluators)
        else {
            todo!()
        };
        let Ok(sig_appr) = serde_json::to_string(&event.content.approvers)
        else {
            todo!()
        };
        let Ok(sig_vali) = serde_json::to_string(&event.content.validators)
        else {
            todo!()
        };
        if let Err(e) = self
            .conn
            .call(move |conn| {
                let _ =
                    conn.execute("INSERT OR REPLACE INTO signatures (subject_id, sn, signatures_eval, signatures_appr, signatures_vali) VALUES (?1, ?2, ?3, ?4, ?5)", params![subject_id, sn, sig_eval, sig_appr, sig_vali])?;

                Ok(())
            })
            .await
            .map_err(|e| {
                Error::Database(format!(": {}", e))
            })
        {
            println!("{}", e);
            todo!()
        };
    }
}

#[async_trait]
impl Subscriber<Signed<Ledger>> for SqliteLocal {
    async fn notify(&self, event: Signed<Ledger>) {
        let subject_id = event.content.subject_id.to_string();
        let sn = event.content.sn;
        let mut succes = "false".to_owned();
        let data = match event.content.value {
            LedgerValue::Patch(value_wrapper) => value_wrapper.0.to_string(),
            LedgerValue::Error(protocols_error) => {
                let Ok(string) = serde_json::to_string(&protocols_error) else {
                    todo!()
                };
                succes = "true".to_owned();
                string
            }
        };

        if let Err(e) = self
            .conn
            .call(move |conn| {
                let _ =
                    conn.execute("INSERT INTO events (subject_id, sn, data, succes) VALUES (?1, ?2, ?3, ?4)", params![subject_id, sn, data, succes])?;

                Ok(())
            })
            .await
            .map_err(|e| {
                Error::Database(format!(": {}", e))
            })
            {
                println!("{}", e);
                todo!()
        };
    }
}

#[async_trait]
impl Subscriber<SinkDataEvent> for SqliteLocal {
    async fn notify(&self, event: SinkDataEvent) {
        let subject_id = event.metadata.subject_id.to_string();
        let governance_id = event.metadata.governance_id.to_string();
        let genesis_gov_version = event.metadata.genesis_gov_version;
        let namespace = event.metadata.namespace.to_string();
        let schema_id = event.metadata.schema_id;
        let owner = event.metadata.owner.to_string();
        let creator = event.metadata.creator.to_string();
        let active = event.metadata.active.to_string();
        let sn = event.metadata.sn;
        let properties = event.metadata.properties.0.to_string();

        // let sql = "CREATE TABLE IF NOT EXISTS subjects (subject_id TEXT NOT NULL, governance_id TEXT NOT NULL, genesis_gov_version INTEGER NOT NULL, namespace TEXT NOT NULL, schema_id TEXT NOT NULL, owner TEXT NOT NULL, creator TEXT NOT NULL, active TEXT NOT NULL, sn INTEGER NOT NULL, properties TEXT NOT NULL, PRIMARY KEY (subject_id))";
        if let Err(e) = self
        .conn
        .call(move |conn| {
            let _ =
                conn.execute("INSERT OR REPLACE INTO subjects (subject_id, governance_id, genesis_gov_version, namespace, schema_id, owner, creator, active, sn, properties) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", params![subject_id, governance_id, genesis_gov_version, namespace, schema_id, owner, creator, active, sn, properties])?;

            Ok(())
        })
        .await
        .map_err(|e| {
            Error::Database(format!(": {}", e))
        })
        {
            println!("{}", e);
            todo!()
    };
    }
}
