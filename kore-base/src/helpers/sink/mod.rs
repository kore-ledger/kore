// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use reqwest::Client;
use rush::Subscriber;
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::{
    config::SinkServer,
    error::Error,
    subject::sinkdata::{SinkDataEvent, SinkDataMessage, SinkTypes},
};

const TARGET_SINK: &str = "Kore-Helper-Sink";

#[derive(Deserialize, Debug, Clone)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

pub async fn obtain_token(auth: &str, username: &str, password: &str) -> Result<TokenResponse, Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| Error::Sink(format!("Can not build reqwest client: {e}")))?; 

    let res = client
        .post(auth)
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .await
        .map_err(|e| Error::Sink(format!("Can not obtain auth token: {e}")))?;

    let res = res
        .error_for_status()
        .map_err(|e| Error::Sink(format!("Auth token endpoint returned error: {e}")))?;

    res.json::<TokenResponse>()
        .await
        .map_err(|e| Error::Sink(format!("Can not parse OAuth 2.0 token response: {e}")))
}

#[derive(Clone)]
pub struct KoreSink {
    sinks: BTreeMap<String, Vec<SinkServer>>,
    token: Option<Arc<RwLock<TokenResponse>>>,
    auth: String,
    username: String,
    password: String,
}

impl KoreSink {
    pub fn new(
        sinks: BTreeMap<String, Vec<SinkServer>>,
        token: Option<TokenResponse>,
        auth: &str,
        username: &str,
        password: &str,
    ) -> Self {
        Self {
            sinks,
            token: token.map(|t| Arc::new(RwLock::new(t))),
            auth: auth.to_owned(),
            username: username.to_owned(),
            password: password.to_owned(),
        }
    }

    fn server_wants_event(server: &SinkServer, event: &SinkDataMessage) -> bool {
        server.events.contains(&SinkTypes::All)
            || SinkTypes::try_from(event.clone())
                .map(|t| server.events.contains(&t))
                .unwrap_or_else(|_| {
                    unreachable!("We return early on UpdateState; other variants must be supported")
                })
    }

    fn build_url(template: &str, subject_id: &str, schema_id: &str) -> String {
        template
            .replace("{{subject-id}}", subject_id)
            .replace("{{schema-id}}", schema_id)
    }

    async fn current_auth_header(&self) -> Option<String> {
        let arc = self.token.as_ref()?;
        let token = arc.read().await;
        Some(format!("{} {}", token.token_type, token.access_token))
    }

    async fn refresh_token(&self) -> Option<TokenResponse> {
        match obtain_token(&self.auth, &self.username, &self.password).await {
            Ok(t) => Some(t),
            Err(e) => {
                error!(target: TARGET_SINK, "Can not get new auth token: {e}");
                None
            }
        }
    }

    async fn send_once(
        client: &Client,
        url: &str,
        event: &SinkDataMessage,
        auth_header: Option<&str>,
    ) -> Result<(), reqwest::Error> {
        let req = if let Some(h) = auth_header {
            client.post(url).header("Authorization", h).json(event)
        } else {
            client.post(url).json(event)
        };

        let res = req.send().await?;
        res.error_for_status_ref()?;
        Ok(())
    }

    async fn send_with_retry_on_401(
        &self,
        client: &Client,
        url: &str,
        event: &SinkDataMessage,
        server_requires_auth: bool,
    ) {
        // 1) Primer intento (con header si tenemos token)
        let header = if server_requires_auth {
            self.current_auth_header().await
        } else {
            None
        };

        match Self::send_once(client, url, event, header.as_deref()).await {
            Ok(_) => {
                info!(target: TARGET_SINK, "The information was sent to the sink");
            }
            Err(e) => {
                // Manejo específico por status
                if let Some(status) = e.status() {
                    match status.as_u16() {
                        422 => {
                            warn!(target: TARGET_SINK, "The sink was expecting another type of data");
                        }
                        401 if server_requires_auth && self.token.is_some() => {
                            warn!(target: TARGET_SINK, "Authentication error on sink; trying to refresh token");
                            // 2) Refrescar token y reintentar una vez
                            if let Some(new_token) = self.refresh_token().await {
                                if let Some(arc) = &self.token {
                                    *arc.write().await = new_token.clone();
                                }
                                info!(target: TARGET_SINK, "A new token has been obtained, retrying the original request.");
                                let new_header =
                                    format!("{} {}", new_token.token_type, new_token.access_token);

                                match Self::send_once(client, url, event, Some(&new_header)).await {
                                    Ok(_) => {
                                        info!(target: TARGET_SINK, "The information was sent to the sink");
                                    }
                                    Err(e2) => {
                                        if e2.status().map(|s| s.as_u16()) == Some(422) {
                                            warn!(target: TARGET_SINK, "The sink was expecting another type of data");
                                        } else {
                                            error!(target: TARGET_SINK, "The information was not sent to the sink: {e2}");
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            error!(target: TARGET_SINK, "The information was not sent to the sink: {e}");
                        }
                    }
                } else {
                    // Error de red, DNS, etc.
                    error!(target: TARGET_SINK, "Can not send post to sink: {e}");
                }
            }
        }
    }
}

#[async_trait]
impl Subscriber<SinkDataEvent> for KoreSink {
    async fn notify(&self, event: SinkDataEvent) {
        let event = event.0;

        if matches!(event, SinkDataMessage::UpdateState(..)) {
            return;
        }

        let (subject_id, schema_id) = event.get_subject_schema();
        let Some(servers) = self.sinks.get(&schema_id) else {
            return;
        };
        if servers.is_empty() {
            return;
        }

        let client = Client::new();

        for server in servers {
            if !Self::server_wants_event(server, &event) {
                continue;
            }

            let url = Self::build_url(&server.url, &subject_id, &schema_id);
            let requires_auth = server.auth;

            self.send_with_retry_on_401(&client, &url, &event, requires_auth)
                .await;
        }
    }
}
