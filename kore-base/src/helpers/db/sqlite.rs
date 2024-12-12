use std::str::FromStr;

use actor::{ActorRef, Subscriber};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio_rusqlite::{params, Connection, OpenFlags};
use tracing::error;

use crate::approval::approver::ApproverEvent;
use crate::error::Error;
use crate::external_db::{DBManager, DBManagerMessage, DeleteTypes};
use crate::helpers::db::EventDB;
use crate::model::event::{Ledger, LedgerValue};
use crate::request::manager::RequestManagerEvent;
use crate::request::state::RequestManagerState;
use crate::request::RequestHandlerEvent;
use crate::subject::event::LedgerEventEvent;
use crate::subject::sinkdata::SinkDataEvent;
use crate::Signed;

use super::{Paginator, Querys, SignaturesDB, SubjectDB};

const TARGET_SQLITE: &str = "Kore-Helper-DB-Sqlite";

#[derive(Clone)]
pub struct SqliteLocal {
    manager: ActorRef<DBManager>,
    conn: Connection,
}

#[async_trait]
impl Querys for SqliteLocal {
    async fn get_signatures(
        &self,
        subject_id: &str,
    ) -> Result<Value, Error> {
        let subject_id = subject_id.to_owned();

        let signatures: SignaturesDB = match self
            .conn
            .call(move |conn| {
                let sql = "SELECT * FROM signatures WHERE subject_id = ?1";

                match conn.query_row(sql, params![subject_id], |row| {
                    Ok(SignaturesDB {
                        subject_id: row.get(0)?,
                        sn: row.get(1)?,
                        signatures_eval: row.get(2)?,
                        signatures_appr: row.get(3)?,
                        signatures_vali: row.get(4)?,
                    })
                }) {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(signatures) => signatures,
            Err(e) => return Err(Error::ExtDB(e.to_string())),
        };
        /*
        pub subject_id: String,
    pub sn: u64,
    pub signatures_eval: String,
    pub signatures_appr: String,
    pub signatures_vali: String,
     */
        let signatures = json!({
            "subject_id": signatures.subject_id,
            "sn": signatures.sn,
            "signatures_eval": Value::from_str(&signatures.signatures_eval).map_err(|e| Error::ExtDB(format!("Can not convert signatures_eval into Value: {}", e)))?,
            "signatures_appr": Value::from_str(&signatures.signatures_appr).map_err(|e| Error::ExtDB(format!("Can not convert signatures_appr into Value: {}", e)))?,
            "signatures_vali": Value::from_str(&signatures.signatures_vali).map_err(|e| Error::ExtDB(format!("Can not convert signatures_vali into Value: {}", e)))?
        });

        Ok(signatures)
    }

    async fn get_subject_state(
        &self,
        subject_id: &str,
    ) -> Result<Value, Error> {
        let subject_id = subject_id.to_owned();

        let subject: SubjectDB = match self
            .conn
            .call(move |conn| {
                let sql = "SELECT * FROM subjects WHERE subject_id = ?1";

                match conn.query_row(sql, params![subject_id], |row| {
                    Ok(SubjectDB {
                        subject_id: row.get(0)?,
                        governance_id: row.get(1)?,
                        genesis_gov_version: row.get(2)?,
                        namespace: row.get(3)?,
                        schema_id: row.get(4)?,
                        owner: row.get(5)?,
                        creator: row.get(6)?,
                        active: row.get(7)?,
                        sn: row.get(8)?,
                        properties: row.get(9)?,
                    })
                }) {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(subject) => subject,
            Err(e) => return Err(Error::ExtDB(e.to_string())),
        };

        let subject = json!({
            "subject_id": subject.subject_id,
            "governance_id": subject.governance_id,
            "genesis_gov_version": subject.genesis_gov_version,
            "namespace": subject.namespace,
            "schema_id": subject.schema_id,
            "owner": subject.owner,
            "creator": subject.creator,
            "active": bool::from_str(&subject.active).map_err(|e| Error::ExtDB(format!("Can not convert active into bool: {}", e)))?,
            "sn": subject.sn,
            "properties": Value::from_str(&subject.properties).map_err(|e| Error::ExtDB(format!("Can not convert properties into Value: {}", e)))?
        });

        Ok(subject)
    }

    async fn get_events(
        &self,
        subject_id: &str,
        quantity: Option<u64>,
        page: Option<u64>,
    ) -> Result<Value, Error> {
        let mut quantity = quantity.unwrap_or(50);
        let mut page = page.unwrap_or(1);
        if page == 0 {
            page = 1;
        }
        if quantity == 0 {
            quantity = 1;
        }

        let subject_id_cloned = subject_id.to_owned();

        let total: u64 = match self
            .conn
            .call(move |conn| {
                let sql = "SELECT COUNT(*) FROM events WHERE subject_id = ?1";

                match conn.query_row(sql, params![subject_id_cloned], |row| {
                    row.get(0)
                }) {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(state) => state,
            Err(e) => return Err(Error::ExtDB(e.to_string())),
        };

        if total == 0 {
            return Err(Error::ExtDB(format!("There is no event for subject {}", subject_id)));
        }

        let mut pages = if total % quantity == 0 {
            total / quantity
        } else {
            total / quantity + 1
        };

        if pages == 0 {
            pages = 1;
        }

        if page > pages {
            page = pages
        }

        let offset = (page - 1) * quantity;

        let subject_id = subject_id.to_owned();

        let events = match self
            .conn
            .call(move |conn| {
                let sql = "SELECT * FROM events WHERE subject_id = ?1 LIMIT ?2 OFFSET ?3";

                let mut stmt = conn.prepare(sql)?;
                // subject_id TEXT NOT NULL, sn INTEGER NOT NULL, data TEXT NOT NULL, succes TEXT
                let events = stmt.query_map(params![subject_id, quantity, offset], |row| {
                    Ok(EventDB {
                        subject_id: row.get(0)?,
                        sn: row.get(1)?,
                        data: row.get(2)?,
                        event_req: row.get(3)?,
                        succes: row.get(4)?,
                    })
                })?.map(|x| {
                    match x {
                        Ok(event) => Ok(json!({
                            "subject_id": event.subject_id,
                            "sn": event.sn,
                            "data": Value::from_str(&event.data).map_err(|e| tokio_rusqlite::Error::Other(Box::new(Error::ExtDB(format!("Can not convert event_req into Value: {}", e)))))?,
                            "event_req": Value::from_str(&event.event_req).map_err(|e| tokio_rusqlite::Error::Other(Box::new(Error::ExtDB(format!("Can not convert event_req into Value: {}", e)))))?,
                            "succes": bool::from_str(&event.succes).map_err(|e| tokio_rusqlite::Error::Other(Box::new(Error::ExtDB(format!("Can not convert succes into bool: {}", e)))))?,
                        })),
                        Err(error) => Err(tokio_rusqlite::Error::Rusqlite(error)),
                    }
                }).collect::<std::result::Result<Vec<Value>, tokio_rusqlite::Error>>()?;

                Ok(events)
            })
            .await
        {
            Ok(events) => events,
            Err(e) => return Err(Error::ExtDB(e.to_string())),
        };

        let prev = if page <= 1 { None } else { Some(page - 1) };

        let next = if page < pages { Some(page + 1) } else { None };
        let paginator = Paginator { pages, next, prev };

        Ok(json!({
            "events": events,
            "paginator": paginator
        }))
    }

    async fn get_request_id_status(
        &self,
        request_id: &str,
    ) -> Result<String, Error> {
        let request_id = request_id.to_owned();
        let state: String = match self
            .conn
            .call(move |conn| {
                let sql = "SELECT state FROM request WHERE id = ?1";

                match conn.query_row(sql, params![request_id], |row| row.get(0))
                {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(state) => state,
            Err(e) => return Err(Error::ExtDB(e.to_string())),
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
            return Err(Error::ExtDB(e.to_string()));
        };

        Ok(())
    }

    async fn get_approve_req(
        &self,
        subject_id: &str,
    ) -> Result<Value, Error> {
        let subject_id = subject_id.to_owned();
        let approve: (String, String) = match self
            .conn
            .call(move |conn| {
                let sql =
                    "SELECT data, state FROM approval WHERE subject_id = ?1";

                match conn.query_row(sql, params![subject_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                }) {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(state) => state,
            Err(e) => return Err(Error::ExtDB(e.to_string())),
        };

        let approve = json!({
            "request": Value::from_str(&approve.0).map_err(|e| Error::ExtDB(format!("Can not convert data into Value: {}", e)))?,
            "state": approve.1
        });

        Ok(approve)
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

                match conn.query_row(sql, params![subject_id], |row| row.get(0))
                {
                    Ok(result) => Ok(result),
                    Err(e) => Err(tokio_rusqlite::Error::Rusqlite(e)),
                }
            })
            .await
        {
            Ok(validators) => validators,
            Err(e) => return Err(Error::ExtDB(e.to_string())),
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
                    Error::ExtDB(format!("SQLite fail open connection: {}", e))
                })?;

        conn.call(|conn| {
            let sql = "CREATE TABLE IF NOT EXISTS request (id TEXT NOT NULL, state TEXT NOT NULL, PRIMARY KEY (id))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS approval (subject_id TEXT NOT NULL, data TEXT NOT NULL, state TEXT NOT NULL, PRIMARY KEY (subject_id))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS validations (subject_id TEXT NOT NULL, validators TEXT NOT NULL, PRIMARY KEY (subject_id))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS events (subject_id TEXT NOT NULL, sn INTEGER NOT NULL, data TEXT NOT NULL, event_req TEXT NOT NULL, succes TEXT NOT NULL, PRIMARY KEY (subject_id, sn))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS subjects (subject_id TEXT NOT NULL, governance_id TEXT NOT NULL, genesis_gov_version INTEGER NOT NULL, namespace TEXT NOT NULL, schema_id TEXT NOT NULL, owner TEXT NOT NULL, creator TEXT NOT NULL, active TEXT NOT NULL, sn INTEGER NOT NULL, properties TEXT NOT NULL, PRIMARY KEY (subject_id))";
            let _ = conn.execute(sql, ())?;

            let sql = "CREATE TABLE IF NOT EXISTS signatures (subject_id TEXT NOT NULL, sn INTEGER NOT NULL, signatures_eval TEXT NOT NULL, signatures_appr TEXT NOT NULL, signatures_vali TEXT NOT NULL, PRIMARY KEY (subject_id))";
            let _ = conn.execute(sql, ())?;
            Ok(())
        }).await.map_err(|e| Error::ExtDB(format!("Can not create request table: {}",e)))?;

        Ok(SqliteLocal { conn, manager })
    }
}

#[async_trait]
impl Subscriber<RequestManagerEvent> for SqliteLocal {
    async fn notify(&self, event: RequestManagerEvent) {
        let state = match event.state {
            RequestManagerState::Starting => return,
            RequestManagerState::Reboot => "In Reboot".to_owned(),
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
                Error::ExtDB(format!("Can not update request state: {}", e))
            })
        {
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
            }
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
                Error::ExtDB(format!(
                    "Update request_id {} state {}: {}",
                    id, state, e
                ))
            })
        {
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
            }
        };

        if state == "Finish" {
            if let Err(e) = self
                .manager
                .tell(DBManagerMessage::Delete(DeleteTypes::Request { id }))
                .await
            {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
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
                    .map_err(|e| Error::ExtDB(format!(": {}", e)))
                {
                    if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await {
                        error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
                    }
                };
            }
            ApproverEvent::SafeState {
                subject_id,
                request,
                state,
                ..
            } => {
                let Ok(request) = serde_json::to_string(&request)
                else {
                    let e = Error::ExtDB(
                        "Can not Serialize request as String".to_owned(),
                    );
                    if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
                    }
                    return;
                };

                if let Err(e) = self
                    .conn
                    .call(move |conn| {
                        let _ =
                            conn.execute("INSERT OR REPLACE INTO approval (subject_id, data, state) VALUES (?1, ?2, ?3)", params![subject_id, request, state.to_string()])?;

                        Ok(())
                    })
                    .await
                    .map_err(|e| {
                        Error::ExtDB(format!(": {}", e))
                    })
                {
                    if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await {
                        error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
                    }
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
                    let e = Error::ExtDB(
                        "Can not Serialize validators as String".to_owned(),
                    );
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
                    }
                    return;
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
                        Error::ExtDB(format!(": {}", e))
                    })
                {
                    if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await {
                        error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
                    }
                };

                event
            }
            LedgerEventEvent::WithOutVal { event } => event,
        };
        let sn = event.content.sn;
        let subject_id = event.content.subject_id.to_string();
        let Ok(sig_eval) = serde_json::to_string(&event.content.evaluators)
        else {
            let e = Error::ExtDB(
                "Can not Serialize evaluators as String".to_owned(),
            );
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
            }
            return;
        };
        let Ok(sig_appr) = serde_json::to_string(&event.content.approvers)
        else {
            let e = Error::ExtDB(
                "Can not Serialize approvers as String".to_owned(),
            );
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
            }
            return;
        };
        let Ok(sig_vali) = serde_json::to_string(&event.content.validators)
        else {
            let e = Error::ExtDB(
                "Can not Serialize validators as String".to_owned(),
            );
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
            }
            return;
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
                Error::ExtDB(format!(": {}", e))
            })
        {
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
            }
        };
    }
}

#[async_trait]
impl Subscriber<Signed<Ledger>> for SqliteLocal {
    async fn notify(&self, event: Signed<Ledger>) {
        let subject_id = event.content.subject_id.to_string();
        let sn = event.content.sn;
        let succes;
        let Ok(event_req) =
            serde_json::to_string(&json!(event.content.event_request.content))
        else {
            let e = Error::ExtDB(
                "Can not Serialize protocols_error as String".to_owned(),
            );
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
            }
            return;
        };
        let data = match event.content.value {
            LedgerValue::Patch(value_wrapper) => {
                succes = "true".to_owned();
                value_wrapper.0.to_string()
            }
            LedgerValue::Error(protocols_error) => {
                let Ok(string) = serde_json::to_string(&protocols_error) else {
                    let e = Error::ExtDB(
                        "Can not Serialize protocols_error as String"
                            .to_owned(),
                    );
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
                    }
                    return;
                };
                succes = "false".to_owned();
                string
            }
        };

        if let Err(e) = self
            .conn
            .call(move |conn| {
                let _ =
                    conn.execute("INSERT INTO events (subject_id, sn, data, event_req, succes) VALUES (?1, ?2, ?3, ?4, ?5)", params![subject_id, sn, data, event_req, succes])?;

                Ok(())
            })
            .await
            .map_err(|e| {
                Error::ExtDB(format!(": {}", e))
            })
            {
                if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await {
                    error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
                }
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

        if let Err(e) = self
        .conn
        .call(move |conn| {
            let _ =
                conn.execute("INSERT OR REPLACE INTO subjects (subject_id, governance_id, genesis_gov_version, namespace, schema_id, owner, creator, active, sn, properties) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", params![subject_id, governance_id, genesis_gov_version, namespace, schema_id, owner, creator, active, sn, properties])?;
            Ok(())
        })
        .await
        .map_err(|e| {
            Error::ExtDB(e.to_string())
        })
        {
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await {
                error!(TARGET_SQLITE, "Can no send message to DBManager actor: {}", e);
            }
    };
    }
}
