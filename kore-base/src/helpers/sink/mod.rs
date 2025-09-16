// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeMap;

use actor::Subscriber;
use async_trait::async_trait;
use tracing::{error, info, warn};

use crate::subject::sinkdata::SinkDataEvent;

const TARGET_SINK: &str = "Kore-Helper-Sink";

#[derive(Clone)]
pub struct KoreSink {
    sinks: BTreeMap<String, String>,
}

impl KoreSink {
    pub fn new(sinks: BTreeMap<String, String>) -> Self {
        Self { sinks }
    }
}

#[async_trait]
impl Subscriber<SinkDataEvent> for KoreSink {
    async fn notify(&self, event: SinkDataEvent) {
        let SinkDataEvent::PublishFact {
            schema_id,
            fact_req,
        } = event
        else {
            return;
        };

        let Some(sink) = self.sinks.get(&schema_id).cloned() else {
            return;
        };

        if sink.is_empty() {
            return;
        }

        let sink =
            sink.replace("{{subject-id}}", &fact_req.subject_id.to_string());
        let sink = sink.replace("{{schema-id}}", &schema_id);

        let client = reqwest::Client::new();
        match client.post(sink).json(&fact_req.payload.0).send().await {
            Ok(res) => {
                if let Err(e) = res.error_for_status() {
                    if let Some(status) = e.status()
                        && status.as_u16() == 422
                    {
                        warn!(
                            TARGET_SINK,
                            "The sink was expecting another type of data"
                        );
                        return;
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
