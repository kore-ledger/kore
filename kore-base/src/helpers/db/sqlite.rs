// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use actor::{ActorRef, Subscriber};
use async_trait::async_trait;
use identity::identifier::KeyIdentifier;
use rusqlite::{Connection, OpenFlags, params};
use serde_json::{Value, json};
use tracing::error;

use crate::Signed;
use crate::approval::approver::ApproverEvent;
use crate::error::Error;
use crate::external_db::{DBManager, DBManagerMessage, DeleteTypes};
use crate::helpers::db::common::{ApproveInfo, EventDB};
use crate::model::event::{Ledger, LedgerValue, ProtocolsSignatures};
use crate::request::RequestHandlerEvent;
use crate::request::manager::RequestManagerEvent;
use crate::request::types::RequestManagerState;
use crate::subject::event::LedgerEventEvent;
use crate::subject::sinkdata::SinkDataEvent;
use crate::subject::validata::ValiDataEvent;

use super::common::{
    EventInfo, Paginator, PaginatorEvents, SignaturesDB, SignaturesInfo,
    SubjectDB, SubjectInfo,
};
use super::{Querys, RequestInfo};

const TARGET_SQLITE: &str = "Kore-Helper-DB-Sqlite";

#[derive(Clone)]
pub struct SqliteLocal {
    manager: ActorRef<DBManager>,
    conn: Arc<Mutex<Connection>>,
}

#[async_trait]
impl Querys for SqliteLocal {
    async fn get_signatures(
        &self,
        subject_id: &str,
    ) -> Result<SignaturesInfo, Error> {
        let subject_id = subject_id.to_owned();

        let signatures: SignaturesDB = {
            if let Ok(conn) = self.conn.lock() {
                let sql = "SELECT * FROM signatures WHERE subject_id = ?1";

                conn.query_row(sql, params![subject_id], |row| {
                    Ok(SignaturesDB {
                        subject_id: row.get(0)?,
                        sn: row.get(1)?,
                        signatures_eval: row.get(2)?,
                        signatures_appr: row.get(3)?,
                        signatures_vali: row.get(4)?,
                    })
                })
                .map_err(|e| Error::ExtDB(e.to_string()))?
            } else {
                return Err(Error::ExtDB(
                    "Can not lock mutex connection with DB".to_owned(),
                ));
            }
        };

        let signatures_eval = if let Some(sig_eval) = signatures.signatures_eval
        {
            Some(serde_json::from_str(&sig_eval).map_err(|e| Error::ExtDB(format!("Can not convert signatures_eval into Option<HashSet<ProtocolsSignaturesInfo>>: {}", e)))?)
        } else {
            None
        };

        let signatures_appr = if let Some(sig_appr) = signatures.signatures_appr
        {
            Some(serde_json::from_str(&sig_appr).map_err(|e| Error::ExtDB(format!("Can not convert signatures_appr into Option<HashSet<ProtocolsSignaturesInfo>>: {}", e)))?)
        } else {
            None
        };

        Ok(SignaturesInfo {
            subject_id: signatures.subject_id,
            sn: signatures.sn,
            signatures_eval,
            signatures_appr,
            signatures_vali: serde_json::from_str(&signatures.signatures_vali).map_err(|e| Error::ExtDB(format!("Can not convert signatures_vali into HashSet<ProtocolsSignaturesInfo>: {}", e)))?
        })
    }

    async fn get_subject_state(
        &self,
        subject_id: &str,
    ) -> Result<SubjectInfo, Error> {
        let subject_id = subject_id.to_owned();

        let subject: SubjectDB = {
            if let Ok(conn) = self.conn.lock() {
                let sql = "SELECT * FROM subjects WHERE subject_id = ?1";

                conn.query_row(sql, params![subject_id], |row| {
                    Ok(SubjectDB {
                        name: row.get(0)?,
                        description: row.get(1)?,
                        subject_id: row.get(2)?,
                        governance_id: row.get(3)?,
                        genesis_gov_version: row.get(4)?,
                        namespace: row.get(5)?,
                        schema_id: row.get(6)?,
                        owner: row.get(7)?,
                        creator: row.get(8)?,
                        active: row.get(9)?,
                        sn: row.get(10)?,
                        properties: row.get(11)?,
                        new_owner: row.get(12)?,
                    })
                })
                .map_err(|e| Error::ExtDB(e.to_string()))?
            } else {
                return Err(Error::ExtDB(
                    "Can not lock mutex connection with DB".to_owned(),
                ));
            }
        };

        Ok(SubjectInfo {
            name: subject.name.unwrap_or_default(),
            description: subject.description.unwrap_or_default(),
            subject_id: subject.subject_id,
            governance_id: subject.governance_id,
            genesis_gov_version: subject.genesis_gov_version,
            namespace: subject.namespace,
            schema_id: subject.schema_id,
            owner: subject.owner,
            creator: subject.creator,
            active: bool::from_str(&subject.active).map_err(|e| {
                Error::ExtDB(format!("Can not convert active into bool: {}", e))
            })?,
            sn: subject.sn,
            properties: Value::from_str(&subject.properties).map_err(|e| {
                Error::ExtDB(format!(
                    "Can not convert properties into Value: {}",
                    e
                ))
            })?,
            new_owner: subject.new_owner,
        })
    }

    async fn get_events(
        &self,
        subject_id: &str,
        quantity: Option<u64>,
        page: Option<u64>,
    ) -> Result<PaginatorEvents, Error> {
        let mut quantity = quantity.unwrap_or(50);
        let mut page = page.unwrap_or(1);
        if page == 0 {
            page = 1;
        }
        if quantity == 0 {
            quantity = 1;
        }

        let subject_id_cloned = subject_id.to_owned();

        let sql = "SELECT COUNT(*) FROM events WHERE subject_id = ?1";

        if let Ok(conn) = self.conn.lock() {
            let total: u64 = conn
                .query_row(sql, params![subject_id_cloned], |row| row.get(0))
                .map_err(|e| Error::ExtDB(e.to_string()))?;

            if total == 0 {
                return Err(Error::ExtDB(format!(
                    "There is no event for subject {}",
                    subject_id
                )));
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

            let sql =
                "SELECT * FROM events WHERE subject_id = ?1 LIMIT ?2 OFFSET ?3";
            let mut stmt =
                conn.prepare(sql).map_err(|e| Error::ExtDB(e.to_string()))?;

            let events = stmt.query_map(params![subject_id, quantity, offset], |row| {
                    Ok(EventDB {
                        subject_id: row.get(0)?,
                        sn: row.get(1)?,
                        patch: row.get(2)?,
                        error: row.get(3)?,
                        event_req: row.get(4)?,
                        succes: row.get(5)?,
                    })
                }).map_err(|e| Error::ExtDB(e.to_string()))?.map(|x| {
                    let event = x.map_err(|e| Error::ExtDB(e.to_string()))?;

                    let patch = if let Some(patch) = event.patch {
                        Some(Value::from_str(&patch).map_err(|e| Error::ExtDB(format!("Can not convert patch into Value: {}", e)))?)
                    } else {
                        None
                    };

                    let error =  if let Some(error) = event.error {
                        Some(serde_json::from_str(&error).map_err(|e| Error::ExtDB(format!("Can not convert patch into Value: {}", e)))?)
                    } else {
                        None
                    };
                    Ok(EventInfo { subject_id: event.subject_id, sn: event.sn, patch, error, event_req: serde_json::from_str(&event.event_req).map_err(|e| Error::ExtDB(format!("Can not convert event_req into EventRequestInfo: {}", e)))?, succes: bool::from_str(&event.succes).map_err(|e| Error::ExtDB(format!("Can not convert succes into bool: {}", e)))?})
                }).collect::<std::result::Result<Vec<EventInfo>, Error>>()?;

            let prev = if page <= 1 { None } else { Some(page - 1) };

            let next = if page < pages { Some(page + 1) } else { None };
            let paginator = Paginator { pages, next, prev };

            Ok(PaginatorEvents { paginator, events })
        } else {
            return Err(Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            ));
        }
    }

    async fn get_events_sn(
        &self,
        subject_id: &str,
        sn: u64,
    ) -> Result<EventInfo, Error> {
        let subject_id = subject_id.to_owned();

        let event = {
            if let Ok(conn) = self.conn.lock() {
                let sql =
                    "SELECT * FROM events WHERE subject_id = ?1 AND sn = ?2";

                conn.query_row(sql, params![subject_id, sn], |row| {
                    Ok(EventDB {
                        subject_id: row.get(0)?,
                        sn: row.get(1)?,
                        patch: row.get(2)?,
                        error: row.get(3)?,
                        event_req: row.get(4)?,
                        succes: row.get(5)?,
                    })
                })
                .map_err(|e| Error::ExtDB(e.to_string()))?
            } else {
                return Err(Error::ExtDB(
                    "Can not lock mutex connection with DB".to_owned(),
                ));
            }
        };

        let patch = if let Some(patch) = event.patch {
            Some(Value::from_str(&patch).map_err(|e| {
                Error::ExtDB(format!("Can not convert patch into Value: {}", e))
            })?)
        } else {
            None
        };

        let error = if let Some(error) = event.error {
            Some(serde_json::from_str(&error).map_err(|e| {
                Error::ExtDB(format!("Can not convert patch into Value: {}", e))
            })?)
        } else {
            None
        };

        Ok(EventInfo {
            subject_id: event.subject_id,
            sn: event.sn,
            patch,
            error,
            event_req: serde_json::from_str(&event.event_req).map_err(|e| {
                Error::ExtDB(format!(
                    "Can not convert event_req into EventRequestInfo: {}",
                    e
                ))
            })?,
            succes: bool::from_str(&event.succes).map_err(|e| {
                Error::ExtDB(format!("Can not convert succes into bool: {}", e))
            })?,
        })
    }

    async fn get_first_or_end_events(
        &self,
        subject_id: &str,
        quantity: Option<u64>,
        reverse: Option<bool>,
        sucess: Option<bool>,
    ) -> Result<Vec<EventInfo>, Error> {
        let subject_id = subject_id.to_owned();
        let mut quantity = quantity.unwrap_or(50);
        if quantity == 0 {
            quantity = 1;
        }
        let reverse = reverse.unwrap_or_default();
        let order = if reverse { "DESC" } else { "ASC" };
        let sucess_condition = if let Some(sucess_value) = sucess {
            format!("AND succes = '{}'", sucess_value)
        } else {
            String::default()
        };

        if let Ok(conn) = self.conn.lock() {
            let sql = format!(
                "SELECT * FROM events WHERE subject_id = ?1 {} ORDER BY sn {} LIMIT ?2",
                sucess_condition, order
            );

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| Error::ExtDB(e.to_string()))?;

            stmt.query_map(params![subject_id, quantity], |row| {
                Ok(EventDB {
                    subject_id: row.get(0)?,
                    sn: row.get(1)?,
                    patch: row.get(2)?,
                    error: row.get(3)?,
                    event_req: row.get(4)?,
                    succes: row.get(5)?,
                })
            }).map_err(|e| Error::ExtDB(e.to_string()))?.map(|x| {
                let event = x.map_err(|e| Error::ExtDB(e.to_string()))?;

                let patch = if let Some(patch) = event.patch {
                    Some(Value::from_str(&patch).map_err(|e| Error::ExtDB(format!("Can not convert patch into Value: {}", e)))?)
                } else {
                    None
                };

                let error =  if let Some(error) = event.error {
                    Some(serde_json::from_str(&error).map_err(|e| Error::ExtDB(format!("Can not convert patch into Value: {}", e)))?)
                } else {
                    None
                };
                Ok(EventInfo { subject_id: event.subject_id, sn: event.sn, patch, error, event_req: serde_json::from_str(&event.event_req).map_err(|e| Error::ExtDB(format!("Can not convert event_req into EventRequestInfo: {}", e)))?, succes: bool::from_str(&event.succes).map_err(|e| Error::ExtDB(format!("Can not convert succes into bool: {}", e)))?})
            }).collect::<std::result::Result<Vec<EventInfo>, Error>>()
        } else {
            return Err(Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            ));
        }
    }

    async fn get_request_id_status(
        &self,
        request_id: &str,
    ) -> Result<RequestInfo, Error> {
        let request_id = request_id.to_owned();

        if let Ok(conn) = self.conn.lock() {
            let sql = "SELECT state, version, error FROM request WHERE id = ?1";

            conn.query_row(sql, params![request_id], |row| {
                Ok(RequestInfo {
                    status: row.get(0)?,
                    version: row.get(1)?,
                    error: row.get(2)?,
                })
            })
            .map_err(|e| Error::ExtDB(e.to_string()))
        } else {
            return Err(Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            ));
        }
    }

    async fn del_request(&self, request_id: &str) -> Result<(), Error> {
        let request_id = request_id.to_owned();

        if let Ok(conn) = self.conn.lock() {
            let sql = "DELETE FROM request WHERE id = ?1";

            conn.execute(sql, params![request_id])
                .map_err(|e| Error::ExtDB(e.to_string()))?;
            Ok(())
        } else {
            return Err(Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            ));
        }
    }

    async fn get_approve_req(
        &self,
        subject_id: &str,
    ) -> Result<ApproveInfo, Error> {
        let subject_id = subject_id.to_owned();

        let approve: (String, String) = {
            if let Ok(conn) = self.conn.lock() {
                let sql =
                    "SELECT data, state FROM approval WHERE subject_id = ?1";

                conn.query_row(sql, params![subject_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .map_err(|e| Error::ExtDB(e.to_string()))?
            } else {
                return Err(Error::ExtDB(
                    "Can not lock mutex connection with DB".to_owned(),
                ));
            }
        };

        Ok(ApproveInfo {
            state: approve.1,
            request: serde_json::from_str(&approve.0).map_err(|e| {
                Error::ExtDB(format!(
                    "Can not convert str to ApprovalReqInfo {}",
                    e
                ))
            })?,
        })
    }

    async fn get_last_validators(
        &self,
        subject_id: &str,
    ) -> Result<String, Error> {
        let subject_id = subject_id.to_owned();

        if let Ok(conn) = self.conn.lock() {
            let sql =
                "SELECT validators FROM validations WHERE subject_id = ?1";

            conn.query_row(sql, params![subject_id], |row| row.get(0))
                .map_err(|e| Error::ExtDB(e.to_string()))
        } else {
            return Err(Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            ));
        }
    }
}

impl SqliteLocal {
    pub async fn new(
        path: &str,
        manager: ActorRef<DBManager>,
    ) -> Result<Self, Error> {
        let flags = OpenFlags::default();
        let conn = Connection::open_with_flags(path, flags).map_err(|e| {
            Error::ExtDB(format!("SQLite fail open connection: {}", e))
        })?;

        let sql = "CREATE TABLE IF NOT EXISTS request (id TEXT NOT NULL, state TEXT NOT NULL, version INTEGER NOT NULL, error TEXT, PRIMARY KEY (id))";
        let _ = conn.execute(sql, ()).map_err(|e| {
            Error::ExtDB(format!("Can not create request table: {}", e))
        })?;

        let sql = "CREATE TABLE IF NOT EXISTS approval (subject_id TEXT NOT NULL, data TEXT NOT NULL, state TEXT NOT NULL, PRIMARY KEY (subject_id))";
        let _ = conn.execute(sql, ()).map_err(|e| {
            Error::ExtDB(format!("Can not create approval table: {}", e))
        })?;

        let sql = "CREATE TABLE IF NOT EXISTS validations (subject_id TEXT NOT NULL, validators TEXT NOT NULL, PRIMARY KEY (subject_id))";
        let _ = conn.execute(sql, ()).map_err(|e| {
            Error::ExtDB(format!("Can not create validations table: {}", e))
        })?;

        let sql = "CREATE TABLE IF NOT EXISTS events (subject_id TEXT NOT NULL, sn INTEGER NOT NULL, patch TEXT, error TEXT, event_req TEXT NOT NULL, succes TEXT NOT NULL, PRIMARY KEY (subject_id, sn))";
        let _ = conn.execute(sql, ()).map_err(|e| {
            Error::ExtDB(format!("Can not create events table: {}", e))
        })?;

        let sql = "CREATE TABLE IF NOT EXISTS subjects (name TEXT, description TEXT, subject_id TEXT NOT NULL, governance_id TEXT NOT NULL, genesis_gov_version INTEGER NOT NULL, namespace TEXT NOT NULL, schema_id TEXT NOT NULL, owner TEXT NOT NULL, creator TEXT NOT NULL, active TEXT NOT NULL, sn INTEGER NOT NULL, properties TEXT NOT NULL, new_owner Text, PRIMARY KEY (subject_id))";
        let _ = conn.execute(sql, ()).map_err(|e| {
            Error::ExtDB(format!("Can not create subjects table: {}", e))
        })?;

        let sql = "CREATE TABLE IF NOT EXISTS signatures (subject_id TEXT NOT NULL, sn INTEGER NOT NULL, signatures_eval TEXT, signatures_appr TEXT, signatures_vali TEXT NOT NULL, PRIMARY KEY (subject_id))";
        let _ = conn.execute(sql, ()).map_err(|e| {
            Error::ExtDB(format!("Can not create signatures table: {}", e))
        })?;

        Ok(SqliteLocal {
            conn: Arc::new(Mutex::new(conn)),
            manager,
        })
    }
}

#[async_trait]
impl Subscriber<RequestManagerEvent> for SqliteLocal {
    async fn notify(&self, event: RequestManagerEvent) {
        let sql = match event {
            RequestManagerEvent::UpdateState { id, state } => {
                let state = match state {
                    RequestManagerState::Starting => return,
                    RequestManagerState::Reboot => "In Reboot".to_owned(),
                    RequestManagerState::Evaluation => {
                        "In Evaluation".to_owned()
                    }
                    RequestManagerState::Approval { .. } => {
                        "In Approval".to_owned()
                    }
                    RequestManagerState::Validation { .. } => {
                        "In Validation".to_owned()
                    }
                    RequestManagerState::Distribution { .. } => {
                        "In Distribution".to_owned()
                    }
                };

                format!(
                    "UPDATE request SET state = '{}' WHERE id = '{}'",
                    state, id
                )
            }
            RequestManagerEvent::UpdateVersion { id, version } => {
                format!(
                    "UPDATE request SET version = '{}' WHERE id = '{}'",
                    version, id
                )
            }
        };

        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(&sql, params![]).map_err(async |e| {
                let e = Error::ExtDB(format!(
                    "Can not update request state: {}",
                    e
                ));
                error!(TARGET_SQLITE, "Subscriber<RequestManagerEvent>: {}", e);
                if let Err(e) =
                    self.manager.tell(DBManagerMessage::Error(e)).await
                {
                    error!(
                        TARGET_SQLITE,
                        "Can no send message to DBManager actor: {}", e
                    );
                }
            });
        } else {
            let e = Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            );
            error!(TARGET_SQLITE, "Subscriber<RequestManagerEvent>: {}", e);
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
        }
    }
}

#[async_trait]
impl Subscriber<RequestHandlerEvent> for SqliteLocal {
    async fn notify(&self, event: RequestHandlerEvent) {
        let (id, state, insert, mut error) = match event {
            RequestHandlerEvent::Abort { id, error, .. } => {
                (id, "Abort".to_owned(), false, error)
            }
            RequestHandlerEvent::EventToQueue { id, .. } => {
                (id, "In queue".to_owned(), true, String::default())
            }
            RequestHandlerEvent::Invalid { id, error, .. } => {
                (id, "Invalid".to_owned(), false, error)
            }
            RequestHandlerEvent::FinishHandling { id, .. } => {
                (id, "Finish".to_owned(), false, String::default())
            }
            RequestHandlerEvent::EventToHandling { request_id, .. } => (
                request_id,
                "In Handling".to_owned(),
                false,
                String::default(),
            ),
        };

        let sql = if insert {
            "INSERT INTO request (id, state, version) VALUES (?1, ?2, 0)"
                .to_owned()
        } else {
            if !error.is_empty() {
                error = format!(", error = '{}'", error);
            }
            format!("UPDATE request SET state = ?2{} WHERE id = ?1", error)
        };

        let id_clone = id.clone();
        let state_clone = state.clone();
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(&sql, params![id_clone, state_clone]).map_err(
                async |e| {
                    let e = Error::ExtDB(format!(
                        "Update request_id {} state {}: {}",
                        id, state, e
                    ));
                    error!(
                        TARGET_SQLITE,
                        "Subscriber<RequestHandlerEvent>: {}", e
                    );
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(
                            TARGET_SQLITE,
                            "Can no send message to DBManager actor: {}", e
                        );
                    }
                },
            );
        } else {
            let e = Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            );
            error!(TARGET_SQLITE, "Subscriber<RequestHandlerEvent>: {}", e);
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
        }

        if state == "Finish" {
            if let Err(e) = self
                .manager
                .tell(DBManagerMessage::Delete(DeleteTypes::Request { id }))
                .await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
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

                if let Ok(conn) = self.conn.lock() {
                    let sql =
                        "UPDATE approval SET state = ?1 WHERE subject_id = ?2";

                    let _ = conn.execute(sql, params![response, subject_id]).map_err(async |e| {
                        let e = Error::ExtDB(format!("Can not update approval: {}", e));
                        error!(TARGET_SQLITE, "Subscriber<ApproverEvent> ApproverEvent::ChangeState: {}", e);
                        if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
                        {
                            error!(
                                TARGET_SQLITE,
                                "Can no send message to DBManager actor: {}", e
                            );
                        }
                    });
                } else {
                    let e = Error::ExtDB(
                        "Can not lock mutex connection with DB".to_owned(),
                    );
                    error!(
                        TARGET_SQLITE,
                        "Subscriber<ApproverEvent> ApproverEvent::ChangeState: {}",
                        e
                    );
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(
                            TARGET_SQLITE,
                            "Can no send message to DBManager actor: {}", e
                        );
                    }
                }
            }
            ApproverEvent::SafeState {
                subject_id,
                request,
                state,
                ..
            } => {
                let Ok(request) = serde_json::to_string(&request) else {
                    let e = Error::ExtDB(
                        "Can not Serialize request as String".to_owned(),
                    );
                    error!(
                        TARGET_SQLITE,
                        "Subscriber<ApproverEvent> ApproverEvent::SafeState: {}",
                        e
                    );
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(
                            TARGET_SQLITE,
                            "Can no send message to DBManager actor: {}", e
                        );
                    }
                    return;
                };

                if let Ok(conn) = self.conn.lock() {
                    let sql = "INSERT OR REPLACE INTO approval (subject_id, data, state) VALUES (?1, ?2, ?3)";

                    let _ = conn.execute(sql, params![subject_id, request, state.to_string()]).map_err(async |e| {
                        let e = Error::ExtDB(format!("Can not update approval: {}", e));
                        error!(TARGET_SQLITE, "Subscriber<ApproverEvent> ApproverEvent::SafeState: {}", e);
                        if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
                        {
                            error!(
                                TARGET_SQLITE,
                                "Can no send message to DBManager actor: {}", e
                            );
                        }
                    });
                } else {
                    let e = Error::ExtDB(
                        "Can not lock mutex connection with DB".to_owned(),
                    );
                    error!(
                        TARGET_SQLITE,
                        "Subscriber<ApproverEvent> ApproverEvent::SafeState: {}",
                        e
                    );
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(
                            TARGET_SQLITE,
                            "Can no send message to DBManager actor: {}", e
                        );
                    }
                }
            }
        }
    }
}

#[async_trait]
impl Subscriber<ValiDataEvent> for SqliteLocal {
    async fn notify(&self, event: ValiDataEvent) {
        let subject_id = event.last_proof.subject_id.to_string();

        let validators: HashSet<KeyIdentifier> = event
            .prev_event_validation_response
            .iter()
            .map(|x| match x {
                ProtocolsSignatures::Signature(signature) => {
                    signature.signer.clone()
                }
                ProtocolsSignatures::TimeOut(time_out_response) => {
                    time_out_response.who.clone()
                }
            })
            .collect();

        let Ok(validators) = serde_json::to_string(&validators) else {
            let e = Error::ExtDB(
                "Can not Serialize validators as String".to_owned(),
            );
            error!(
                TARGET_SQLITE,
                "Subscriber<LedgerEventEvent> LedgerEventEvent::WithVal: {}", e
            );
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
            return;
        };

        if let Ok(conn) = self.conn.lock() {
            let sql = "INSERT OR REPLACE INTO validations (subject_id, validators) VALUES (?1, ?2)";

            let _ = conn.execute(sql, params![subject_id, validators]).map_err(async |e| {
                let e = Error::ExtDB(format!("Can not update validations: {}", e));
                error!(TARGET_SQLITE, "Subscriber<LedgerEventEvent> LedgerEventEvent::WithVal: {}", e);
                if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
                {
                    error!(
                        TARGET_SQLITE,
                        "Can no send message to DBManager actor: {}", e
                    );
                }
            });
        } else {
            let e = Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            );
            error!(
                TARGET_SQLITE,
                "Subscriber<LedgerEventEvent> LedgerEventEvent::WithVal: {}", e
            );
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
        }
    }
}

#[async_trait]
impl Subscriber<LedgerEventEvent> for SqliteLocal {
    async fn notify(&self, event: LedgerEventEvent) {
        let sn = event.event.content.sn;
        let subject_id = event.event.content.subject_id.to_string();

        let sig_eval = if let Some(sig_eval) = event.event.content.evaluators {
            let Ok(sig_eval) = serde_json::to_string(&sig_eval) else {
                let e = Error::ExtDB(
                    "Can not Serialize evaluators as String".to_owned(),
                );
                error!(TARGET_SQLITE, "Subscriber<LedgerEventEvent>: {}", e);
                if let Err(e) =
                    self.manager.tell(DBManagerMessage::Error(e)).await
                {
                    error!(
                        TARGET_SQLITE,
                        "Can no send message to DBManager actor: {}", e
                    );
                }
                return;
            };

            Some(sig_eval)
        } else {
            None
        };

        let sig_appr = if let Some(sig_appr) = event.event.content.approvers {
            let Ok(sig_appr) = serde_json::to_string(&sig_appr) else {
                let e = Error::ExtDB(
                    "Can not Serialize approvers as String".to_owned(),
                );
                error!(TARGET_SQLITE, "Subscriber<LedgerEventEvent>: {}", e);
                if let Err(e) =
                    self.manager.tell(DBManagerMessage::Error(e)).await
                {
                    error!(
                        TARGET_SQLITE,
                        "Can no send message to DBManager actor: {}", e
                    );
                }
                return;
            };

            Some(sig_appr)
        } else {
            None
        };

        let Ok(sig_vali) =
            serde_json::to_string(&event.event.content.validators)
        else {
            let e = Error::ExtDB(
                "Can not Serialize validators as String".to_owned(),
            );
            error!(TARGET_SQLITE, "Subscriber<LedgerEventEvent>: {}", e);
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
            return;
        };

        if let Ok(conn) = self.conn.lock() {
            let sql = "INSERT OR REPLACE INTO signatures (subject_id, sn, signatures_eval, signatures_appr, signatures_vali) VALUES (?1, ?2, ?3, ?4, ?5)";

            let _ = conn
                .execute(
                    sql,
                    params![subject_id, sn, sig_eval, sig_appr, sig_vali],
                )
                .map_err(async |e| {
                    let e = Error::ExtDB(format!(
                        "Can not update signatures: {}",
                        e
                    ));
                    error!(
                        TARGET_SQLITE,
                        "Subscriber<LedgerEventEvent>: {}", e
                    );
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(
                            TARGET_SQLITE,
                            "Can no send message to DBManager actor: {}", e
                        );
                    }
                });
        } else {
            let e = Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            );
            error!(TARGET_SQLITE, "Subscriber<LedgerEventEvent>: {}", e);
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
        }
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
            error!(TARGET_SQLITE, "Subscriber<Signed<Ledger>>: {}", e);
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
            return;
        };
        let (patch, error): (Option<String>, Option<String>) = match event
            .content
            .value
        {
            LedgerValue::Patch(value_wrapper) => {
                succes = "true".to_owned();
                (Some(value_wrapper.0.to_string()), None)
            }
            LedgerValue::Error(protocols_error) => {
                let Ok(string) = serde_json::to_string(&protocols_error) else {
                    let e = Error::ExtDB(
                        "Can not Serialize protocols_error as String"
                            .to_owned(),
                    );
                    error!(
                        TARGET_SQLITE,
                        "Subscriber<Signed<Ledger>> LedgerValue::Error: {}", e
                    );
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(
                            TARGET_SQLITE,
                            "Can no send message to DBManager actor: {}", e
                        );
                    }
                    return;
                };
                succes = "false".to_owned();
                (None, Some(string))
            }
        };

        if let Ok(conn) = self.conn.lock() {
            let sql = "INSERT INTO events (subject_id, sn, patch, error, event_req, succes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

            let _ = conn
                .execute(
                    sql,
                    params![subject_id, sn, patch, error, event_req, succes],
                )
                .map_err(async |e| {
                    let e =
                        Error::ExtDB(format!("Can not update events: {}", e));
                    error!(TARGET_SQLITE, "Subscriber<Signed<Ledger>>: {}", e);
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(
                            TARGET_SQLITE,
                            "Can no send message to DBManager actor: {}", e
                        );
                    }
                });
        } else {
            let e = Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            );
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
        }
    }
}

#[async_trait]
impl Subscriber<SinkDataEvent> for SqliteLocal {
    async fn notify(&self, event: SinkDataEvent) {
        let SinkDataEvent::UpdateState(metadata) = event else {
            return ;
        };

        let name = metadata.name;
        let description = metadata.description;
        let subject_id = metadata.subject_id.to_string();
        let governance_id = metadata.governance_id.to_string();
        let genesis_gov_version = metadata.genesis_gov_version;
        let namespace = metadata.namespace.to_string();
        let schema_id = metadata.schema_id;
        let owner = metadata.owner.to_string();
        let creator = metadata.creator.to_string();
        let active = metadata.active.to_string();
        let sn = metadata.sn;
        let properties = metadata.properties.0.to_string();
        let new_owner = metadata
            .new_owner
            .map(|new_owner| new_owner.to_string());

        if let Ok(conn) = self.conn.lock() {
            let sql = "INSERT OR REPLACE INTO subjects (name, description, subject_id, governance_id, genesis_gov_version, namespace, schema_id, owner, creator, active, sn, properties, new_owner) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";

            let _ = conn
                .execute(
                    sql,
                    params![
                        name,
                        description,
                        subject_id,
                        governance_id,
                        genesis_gov_version,
                        namespace,
                        schema_id,
                        owner,
                        creator,
                        active,
                        sn,
                        properties,
                        new_owner
                    ],
                )
                .map_err(async |e| {
                    let e =
                        Error::ExtDB(format!("Can not update subject: {}", e));
                    error!(TARGET_SQLITE, "Subscriber<SinkDataEvent>: {}", e);
                    if let Err(e) =
                        self.manager.tell(DBManagerMessage::Error(e)).await
                    {
                        error!(
                            TARGET_SQLITE,
                            "Can no send message to DBManager actor: {}", e
                        );
                    }
                });
        } else {
            let e = Error::ExtDB(
                "Can not lock mutex connection with DB".to_owned(),
            );
            error!(TARGET_SQLITE, "Subscriber<SinkDataEvent>: {}", e);
            if let Err(e) = self.manager.tell(DBManagerMessage::Error(e)).await
            {
                error!(
                    TARGET_SQLITE,
                    "Can no send message to DBManager actor: {}", e
                );
            }
        }
    }
}
