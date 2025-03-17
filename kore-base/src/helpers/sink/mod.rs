// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use actor::Subscriber;
use async_trait::async_trait;
use tracing::{error, info, warn};

use crate::{
    EventRequest, Signed,
    model::event::{Ledger, LedgerValue},
};

const TARGET_SINK: &str = "Kore-Helper-Sink";

#[derive(Clone)]
pub struct KoreSink {
    sink: String,
}

impl KoreSink {
    pub fn new(sink: String) -> Self {
        Self { sink }
    }
}

#[async_trait]
impl Subscriber<Signed<Ledger>> for KoreSink {
    async fn notify(&self, event: Signed<Ledger>) {
        if self.sink.is_empty() {
            return;
        }

        if let LedgerValue::Error(_) = event.content.value {
            return;
        }

        let subject_id = event.content.subject_id.to_string();
        let EventRequest::Fact(event) = event.content.event_request.content
        else {
            return;
        };

        let client = reqwest::Client::new();
        match client
            .post(format!("{}/{}", self.sink.clone(), subject_id))
            .json(&event.payload.0)
            .send()
            .await
        {
            Ok(res) => {
                if let Err(e) = res.error_for_status() {
                    if let Some(status) = e.status() {
                        if status.as_u16() == 422 {
                            warn!(TARGET_SINK, "The sink was expecting another type of data");
                            return;
                        }
                    }
                    error!(
                        TARGET_SINK,
                        "The information was not sent to the sink: {}", e
                    );
                } else {
                    info!(TARGET_SINK, "The information was sent to the sink")
                };
            }
            Err(e) => {
                error!(TARGET_SINK, "Can not send post to sink: {}", e);
            }
        };
    }
}
