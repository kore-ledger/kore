use std::{collections::HashSet, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Namespace, event::ProtocolsError};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaginatorEvents {
    pub paginator: Paginator,
    pub events: Vec<EventInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRequestInfo {
    pub name: Option<String>,
    pub description: Option<String>,
    pub governance_id: String,
    pub schema_id: String,
    pub namespace: Namespace,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferRequestInfo {
    pub subject_id: String,
    pub new_owner: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfirmRequestInfo {
    pub subject_id: String,
    pub name_old_owner: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EOLRequestInfo {
    pub subject_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FactRequestInfo {
    pub subject_id: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RejectRequestInfo {
    pub subject_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub enum EventRequestInfo {
    Create(CreateRequestInfo),
    Fact(FactRequestInfo),
    Transfer(TransferRequestInfo),
    Confirm(ConfirmRequestInfo),
    EOL(EOLRequestInfo),
    Reject(RejectRequestInfo),
}

impl<'de> Deserialize<'de> for EventRequestInfo {
    fn deserialize<D>(deserializer: D) -> Result<EventRequestInfo, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;

        if let Some(create) = value.get("Create") {
            let namespace = create
                .get("namespace")
                .ok_or_else(|| serde::de::Error::missing_field("namespace"))?;

            return Ok(Self::Create(CreateRequestInfo {
                name: create
                .get("name")
                .and_then(Value::as_str)
                .map(|x| x.to_owned())
                .to_owned(),
                description: create
                .get("description")
                .and_then(Value::as_str)
                .map(|x| x.to_owned())
                .to_owned(),
                governance_id: create
                    .get("governance_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        serde::de::Error::missing_field("governance_id")
                    })?
                    .to_owned(),
                schema_id: create
                    .get("schema_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        serde::de::Error::missing_field("schema_id")
                    })?
                    .to_owned(),
                namespace: serde_json::from_value(namespace.clone())
                    .map_err(|e| serde::de::Error::custom(e.to_string()))?,
            }));
        };

        if let Some(fact) = value.get("Fact") {
            let payload_str = fact
                .get("payload")
                .and_then(Value::as_str)
                .ok_or_else(|| serde::de::Error::missing_field("payload"))?;
            let payload = Value::from_str(payload_str)
                .map_err(serde::de::Error::custom)?;
            return Ok(Self::Fact(FactRequestInfo {
                subject_id: fact
                    .get("subject_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        serde::de::Error::missing_field("subject_id")
                    })?
                    .to_owned(),
                payload,
            }));
        };

        if let Some(transfer) = value.get("Transfer") {
            return Ok(Self::Transfer(TransferRequestInfo {
                subject_id: transfer
                    .get("subject_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        serde::de::Error::missing_field("subject_id")
                    })?
                    .to_owned(),
                new_owner: transfer
                    .get("new_owner")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        serde::de::Error::missing_field("new_owner")
                    })?
                    .to_owned(),
            }));
        };

        if let Some(confirm) = value.get("Confirm") {
            return Ok(Self::Confirm(ConfirmRequestInfo {
                subject_id: confirm
                    .get("subject_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        serde::de::Error::missing_field("subject_id")
                    })?
                    .to_owned(),
                name_old_owner: confirm
                    .get("name_old_owner")
                    .and_then(Value::as_str)
                    .map(|x| x.to_string()),
            }));
        };

        if let Some(reject) = value.get("Reject") {
            return Ok(Self::Reject(RejectRequestInfo {
                subject_id: reject
                    .get("subject_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        serde::de::Error::missing_field("subject_id")
                    })?
                    .to_owned(),
            }));
        }

        if let Some(eol) = value.get("EOL") {
            return Ok(Self::EOL(EOLRequestInfo {
                subject_id: eol
                    .get("subject_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        serde::de::Error::missing_field("subject_id")
                    })?
                    .to_owned(),
            }));
        };

        Err(serde::de::Error::custom("Invalid EventRequest type"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventInfo {
    pub subject_id: String,
    pub sn: u64,
    pub patch: Option<Value>,
    pub error: Option<ProtocolsError>,
    pub event_req: EventRequestInfo,
    pub succes: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignaturesDB {
    pub subject_id: String,
    pub sn: u64,
    pub signatures_eval: Option<String>,
    pub signatures_appr: Option<String>,
    pub signatures_vali: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignaturesInfo {
    pub subject_id: String,
    pub sn: u64,
    pub signatures_eval: Option<HashSet<ProtocolsSignaturesInfo>>,
    pub signatures_appr: Option<HashSet<ProtocolsSignaturesInfo>>,
    pub signatures_vali: HashSet<ProtocolsSignaturesInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProtocolsSignaturesInfo {
    Signature(SignatureInfo),
    TimeOut(TimeOutResponseInfo),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TimeOutResponseInfo {
    pub who: String,
    pub re_trys: u32,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SubjectDB {
    pub name: Option<String>,
    pub description: Option<String>,
    pub subject_id: String,
    pub governance_id: String,
    pub genesis_gov_version: u64,
    pub namespace: String,
    pub schema_id: String,
    pub owner: String,
    pub creator: String,
    pub active: String,
    pub sn: u64,
    pub properties: String,
    pub new_owner: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubjectInfo {
    pub name: String,
    pub description: String,
    pub subject_id: String,
    pub governance_id: String,
    pub genesis_gov_version: u64,
    pub namespace: String,
    pub schema_id: String,
    pub owner: String,
    pub creator: String,
    pub active: bool,
    pub sn: u64,
    pub properties: Value,
    pub new_owner: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventDB {
    pub subject_id: String,
    pub sn: u64,
    pub patch: Option<String>,
    pub error: Option<String>,
    pub event_req: String,
    pub succes: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paginator {
    pub pages: u64,
    pub next: Option<u64>,
    pub prev: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestInfo {
    pub status: String,
    pub version: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApproveInfo {
    pub state: String,
    pub request: ApprovalReqInfo,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApprovalReqInfo {
    /// The signed event request.
    pub event_request: SignedInfo<FactInfo>,
    /// The sequence number of the event.
    pub sn: u64,
    /// The version of the governance contract.
    pub gov_version: u64,
    /// The patch to apply to the state.
    pub patch: Value,
    /// The hash of the state after applying the patch.
    pub state_hash: String,
    /// The hash of the previous event.
    pub hash_prev_event: String,
    /// The hash of the previous event.
    pub subject_id: String,
}

impl<'de> Deserialize<'de> for ApprovalReqInfo {
    fn deserialize<D>(deserializer: D) -> Result<ApprovalReqInfo, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;

        let sn = value
            .get("sn")
            .and_then(Value::as_u64)
            .ok_or_else(|| serde::de::Error::missing_field("sn"))?;
        let gov_version = value
            .get("gov_version")
            .and_then(Value::as_u64)
            .ok_or_else(|| serde::de::Error::missing_field("gov_version"))?;
        let state_hash = value
            .get("state_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::missing_field("state_hash"))?
            .to_owned();
        let hash_prev_event = value
            .get("hash_prev_event")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::missing_field("hash_prev_event"))?
            .to_owned();
        let subject_id = value
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::missing_field("subject_id"))?
            .to_owned();
        let patch_str = value
            .get("patch")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::missing_field("patch"))?;
        let patch =
            Value::from_str(patch_str).map_err(serde::de::Error::custom)?;

        let event_request = value
            .get("event_request")
            .and_then(Value::as_object)
            .ok_or_else(|| serde::de::Error::missing_field("event_request"))?;

        let content =
            event_request
                .get("content")
                .and_then(Value::as_object)
                .ok_or_else(|| serde::de::Error::missing_field("content"))?;
        let fact = content
            .get("Fact")
            .and_then(Value::as_object)
            .ok_or_else(|| serde::de::Error::missing_field("Fact"))?;
        let payload_str = fact
            .get("payload")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::missing_field("payload"))?;
        let payload =
            Value::from_str(payload_str).map_err(serde::de::Error::custom)?;
        let subject_id_fact = fact
            .get("subject_id")
            .and_then(Value::as_str)
            .ok_or_else(|| serde::de::Error::missing_field("subject_id"))?
            .to_owned();

        let signature = serde_json::from_value::<SignatureInfo>(
            event_request
                .get("signature")
                .ok_or_else(|| serde::de::Error::missing_field("signature"))?
                .clone(),
        )
        .map_err(|e| serde::de::Error::custom(e.to_string()))?;

        Ok(Self {
            event_request: SignedInfo {
                content: FactInfo {
                    payload,
                    subject_id: subject_id_fact,
                },
                signature,
            },
            sn,
            gov_version,
            patch,
            state_hash,
            hash_prev_event,
            subject_id,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct FactInfo {
    pub payload: Value,
    pub subject_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SignedInfo<T: Serialize + Clone> {
    /// The data that is signed
    pub content: T,
    /// The signature accompanying the data
    pub signature: SignatureInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SignatureInfo {
    /// Signer identifier
    pub signer: String,
    /// Timestamp of the signature
    pub timestamp: u64,
    /// Hash of the content signed
    pub content_hash: String,
    /// The signature itself
    pub value: String,
}
