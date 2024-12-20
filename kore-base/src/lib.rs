// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later
#![recursion_limit = "256"]
pub mod config;
pub mod error;

pub mod approval;
pub mod auth;
pub mod db;
pub mod distribution;
pub mod evaluation;
pub mod external_db;
pub mod governance;
pub mod helpers;
pub mod model;
pub mod node;
pub mod query;
pub mod request;
pub mod subject;
pub(crate) mod system;
pub mod update;
pub mod validation;

use actor::{ActorPath, ActorRef, Sink};
use approval::approver::ApprovalStateRes;
use async_std::sync::RwLock;
use auth::{Auth, AuthMessage, AuthResponse, AuthWitness};
use config::Config as KoreBaseConfig;
use error::Error;
use governance::json_schema::JsonSchema;
use governance::schema;
use governance::{init::init_state, Governance};
use helpers::db::ExternalDB;
use helpers::network::*;
use identity::identifier::derive::{digest::DigestDerivator, KeyDerivator};
use identity::identifier::DigestIdentifier;
use identity::keys::KeyPair;
use intermediary::Intermediary;
use model::event::Event;
use model::signature::*;
use model::HashId;
use model::ValueWrapper;
use model::{request::*, SignTypesNode};
use network::{Monitor, NetworkWorker};
use node::register::{GovsData, Register, RegisterData, RegisterMessage, RegisterResponse};
use node::{Node, NodeMessage, NodeResponse, SubjectsTypes};
use once_cell::sync::OnceCell;
use prometheus_client::registry::Registry;
use query::{Query, QueryMessage, QueryResponse};
use request::{
    RequestData, RequestHandler, RequestHandlerMessage, RequestHandlerResponse,
};
use serde_json::Value;
use subject::{Subject, SubjectMessage, SubjectResponse};
use system::system;
use tokio_util::sync::CancellationToken;
use tracing::{info, error};
use validation::{Validation, ValidationInfo, ValidationMessage};

use lazy_static::lazy_static;

use std::collections::HashMap;
use std::sync::Mutex;

const TARGET_API: &str = "Kore-Api";

lazy_static! {
    /// The digest derivator for the system.
    pub static ref DIGEST_DERIVATOR: Mutex<DigestDerivator> = Mutex::new(DigestDerivator::Blake3_256);
    /// The key derivator for the system.
    pub static ref KEY_DERIVATOR: Mutex<KeyDerivator> = Mutex::new(KeyDerivator::Ed25519);

    pub static ref CONTRACTS: RwLock<HashMap<String, Vec<u8>>> = {
        let contracts = HashMap::new();

        RwLock::new(contracts)
    };
}

static GOVERNANCE: OnceCell<RwLock<JsonSchema>> = OnceCell::new();

#[derive(Clone)]
pub struct Api {
    peer_id: String,
    controller_id: String,
    request: ActorRef<RequestHandler>,
    node: ActorRef<Node>,
    auth: ActorRef<Auth>,
    query: ActorRef<Query>,
    register: ActorRef<Register>,
}

impl Api {
    /// Creates a new `Api`.
    pub async fn new(
        keys: KeyPair,
        config: KoreBaseConfig,
        registry: &mut Registry,
        password: &str,
        token: &CancellationToken,
    ) -> Result<Self, Error> {
        info!(TARGET_API, "Creating Api");
        let schema = JsonSchema::compile(&schema())?;

        if let Err(_e) = GOVERNANCE.set(RwLock::new(schema)) {
            error!(TARGET_API, "Can not set governance schema");
            #[cfg(test)]
            return Err(Error::System("An error occurred with the governance schema, it could not be initialized globally".to_owned()));
        };

        let system =
            system(config.clone(), password, Some(token.clone())).await?;


            let newtork_monitor = Monitor;
            let newtork_monitor_actor = system
                .create_root_actor("network_monitor", newtork_monitor)
                .await
                .map_err(|e| {
                    error!(TARGET_API, "Can not create network_monitor actor {}", e);
                    Error::System(e.to_string())
                })?;
    
            let mut worker: NetworkWorker<NetworkMessage> = NetworkWorker::new(
                registry,
                keys.clone(),
                config.network.clone(),
                Some(newtork_monitor_actor),
                config.key_derivator,
                token.clone(),
            )
            .map_err(|e| {
                error!(TARGET_API, "Can not create networt {}", e);
                Error::Network(e.to_string())})?;
    
            // Create worker
            let service = Intermediary::new(
                worker.service().sender().clone(),
                KeyDerivator::Ed25519,
                system.clone(),
                token.clone(),
            );
    
            let peer_id = worker.local_peer_id().to_string();
    
            worker.add_helper_sender(service.service().sender());
    
            system.add_helper("network", service).await;
    
            tokio::spawn(async move {
                let _ = worker.run().await;
            });

        let node = Node::new(&keys)?;
        let node_actor = system
            .create_root_actor("node", node)
            .await
            .map_err(|e| {
                error!(TARGET_API, "Can not create node actor {}", e);
                Error::System(e.to_string())
            })?;

        let register: Option<ActorRef<Register>> = system
            .get_actor(&ActorPath::from("/user/node/register"))
            .await;
        let Some(register_actor) = register else {
            error!(TARGET_API, "Can not get register actor");
            return Err(Error::System("Can not get register actor".to_owned()));
        };

        let auth: Option<ActorRef<Auth>> =
            system.get_actor(&ActorPath::from("/user/node/auth")).await;
        let Some(auth_actor) = auth else {
            error!(TARGET_API, "Can not get auth actor");
            return Err(Error::System("Can not get auth actor".to_owned()));
        };

        let request = RequestHandler::new(keys.key_identifier());
        let request_actor = system
            .create_root_actor("request", request)
            .await
            .map_err(|e| {
                error!(TARGET_API, "Can not create request actor {}", e);
                Error::System(e.to_string())})?;
        let Some(ext_db): Option<ExternalDB> =
            system.get_helper("ext_db").await
        else {
            error!(TARGET_API, "Can not get ext_db helper");
            return Err(Error::System("Can not get ext_db helper".to_owned()));
        };

        let sink =
            Sink::new(request_actor.subscribe(), ext_db.get_request_handler());
        system.run_sink(sink).await;

        let query = Query::new(keys.key_identifier());
        let query_actor = system
            .create_root_actor("query", query)
            .await
            .map_err(|e| {
                error!(TARGET_API, "Can not create query actor {}", e);
                Error::System(e.to_string())
            })?;

        Ok(Self {
            controller_id: keys.key_identifier().to_string(),
            peer_id,
            request: request_actor,
            auth: auth_actor,
            node: node_actor,
            query: query_actor,
            register: register_actor,
        })
    }

    pub fn peer_id(&self) -> String {
        self.peer_id.clone()
    }

    pub fn controller_id(&self) -> String {
        self.controller_id.clone()
    }

    /// Request from issuer.
    pub async fn external_request(
        &self,
        request: Signed<EventRequest>,
    ) -> Result<RequestData, Error> {
        let response = self
            .request
            .ask(RequestHandlerMessage::NewRequest { request })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not send external request {}", e);
                Error::RequestHandler(
                    e.to_string(),
                )
            })?;

        match response {
            RequestHandlerResponse::Ok(request_data) => Ok(request_data),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::RequestHandler(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))},
        }
    }

    /// Own request.
    pub async fn own_request(
        &self,
        request: EventRequest,
    ) -> Result<RequestData, Error> {
        let response = self
            .node
            .ask(NodeMessage::SignRequest(SignTypesNode::EventRequest(
                request.clone(),
            )))
            .await.map_err(|e| {
                error!(TARGET_API, "Can not sign request {}", e);
                Error::Node(
                    "The node was unable to sign the request".to_owned(),
                )
            })?;

        let signature = match response {
            NodeResponse::SignRequest(signature) => signature,
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                return Err(Error::Node(
                    "A response was received that was not the expected one"
                        .to_owned(),
                ))
            }
        };

        let signed_event_req = Signed {
            content: request,
            signature,
        };

        let response = self
            .request
            .ask(RequestHandlerMessage::NewRequest {
                request: signed_event_req,
            })
            .await
            .map_err(|e| {
                error!(TARGET_API, "Can not send our request {}", e);
                Error::RequestHandler(e.to_string())
            })?;

        match response {
            RequestHandlerResponse::Ok(request_data) => Ok(request_data),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::RequestHandler(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))},
        }
    }

    pub async fn request_state(
        &self,
        request_id: DigestIdentifier,
    ) -> Result<String, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetRequestState {
                request_id: request_id.to_string(),
            })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not get request state {}", e);
                Error::Query(e.to_string())
            })?;

        match response {
            QueryResponse::RequestState(state) => Ok(state),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Query(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn get_approval(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<Value, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not get approval request {}", e);
                Error::Query(e.to_string())
            })?;

        match response {
            QueryResponse::ApprovalState { data } => {
                Ok(data)
            }
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Query(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn approve(
        &self,
        subject_id: DigestIdentifier,
        state: ApprovalStateRes,
    ) -> Result<String, Error> {
        if let ApprovalStateRes::Obsolete = state {
            error!(TARGET_API, "Invalid approval state");
            return Err(Error::RequestHandler("Invalid approval state".to_owned()));
        }

        let response = self
            .request
            .ask(RequestHandlerMessage::ChangeApprovalState {
                subject_id: subject_id.to_string(),
                state,
            })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not change approve request state: {}", e);
                Error::RequestHandler(e.to_string())
            })?;

        match response {
            RequestHandlerResponse::Response(res) => Ok(res),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::RequestHandler(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn auth_subject(
        &self,
        subject_id: DigestIdentifier,
        witnesses: AuthWitness,
    ) -> Result<String, Error> {
        self
            .auth
            .tell(AuthMessage::NewAuth {
                subject_id,
                witness: witnesses,
            })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not get auth subject: {}", e);
                Error::Auth(
                    format!("Can not get auth subject: {}", e)
                )
            })?;

        Ok("Ok".to_owned())
    }

    pub async fn all_auth_subjects(&self) -> Result<Vec<String>, Error> {
        let response = self.auth.ask(AuthMessage::GetAuths).await
            .map_err(|e| {
                error!(TARGET_API, "Can not get auth subject: {}", e);
                Error::Auth(format!("Can not get auth subjects: {}", e))
            })?;

        match response {
            AuthResponse::Auths { subjects } => Ok(subjects),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Auth(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn witnesses_subject(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<AuthWitness, Error> {
        let response =
            self.auth.ask(AuthMessage::GetAuth { subject_id }).await
            .map_err(|e| {
                error!(TARGET_API, "Can not get witnesses of subject: {}", e);
                Error::Auth(format!("Can not get witnesses of subjects: {}", e))
            })?;

        match response {
            AuthResponse::Witnesses(witnesses) => Ok(witnesses),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Auth(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn delete_auth_subject(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<String, Error> {
        self.auth.tell(AuthMessage::DeleteAuth { subject_id }).await.map_err(|e| {
                error!(TARGET_API, "Can not delete auth of subjects: {}", e);
                Error::Auth(format!("Can not delete auth of subjects: {}", e))
            })?;

        Ok("Ok".to_owned())
    }

    pub async fn update_subject(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<String, Error> {
        let response =
            self.auth.ask(AuthMessage::Update { subject_id }).await
            .map_err(|e| {
                error!(TARGET_API, "Can not update subject: {}", e);
                Error::Auth(format!("Can not update subject: {}", e))
        })?;

        match response {
            AuthResponse::None => Ok("Update in progress".to_owned()),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Auth(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
            }
        }
    }

    pub async fn all_govs(&self, active: Option<bool>) -> Result<Vec<GovsData>, Error> {
        let response = self.register.ask(RegisterMessage::GetGovs { active }).await
        .map_err(|e| {
            error!(TARGET_API, "Can not get resgister governances: {}", e);
            Error::Register(format!("Can not get resgister governances: {}", e))
        })?;

        match response {
            RegisterResponse::Govs { governances } => Ok(governances),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Register(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn all_subjs(
        &self,
        gov_id: DigestIdentifier,
        active: Option<bool>,
        schema: Option<String>,
    ) -> Result<Vec<RegisterData>, Error> {
        let response= self
            .register
            .ask(RegisterMessage::GetSubj {
                gov_id: gov_id.to_string(),
                active,
                schema,
            })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not get resgister subjects: {}", e);
                Error::Register(format!("Can not get resgister subjects: {}", e))
            })?;

        match response {
            RegisterResponse::Subjs { subjects } => Ok(subjects),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Register(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn get_events(
        &self,
        subject_id: DigestIdentifier,
        quantity: Option<u64>,
        page: Option<u64>,
    ) -> Result<Value, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetEvents {
                subject_id: subject_id.to_string(),
                quantity,
                page,
            })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not get events: {}", e);
                Error::Query(format!("Can not get events: {}", e))
            })?;

        match response {
            QueryResponse::Events { data } => {
                Ok(data)
            }
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Query(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn get_subject(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<Value, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetSubject {
                subject_id: subject_id.to_string(),
            })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not get subject: {}", e);
                Error::Query(format!("Can not get subject: {}", e))
            })?;

        match response {
            QueryResponse::Subject { subject } => Ok(subject),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Query(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }

    pub async fn get_signatures(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<Value, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetSignatures {
                subject_id: subject_id.to_string(),
            })
            .await.map_err(|e| {
                error!(TARGET_API, "Can not get signatures: {}", e);
                Error::Query(format!("Can not get signatures: {}", e))  
            })?;

        match response {
            QueryResponse::Signatures { signatures } => Ok(signatures),
            _ => {
                error!(TARGET_API, "A response was received that was not the expected one");
                Err(Error::Query(
                "A response was received that was not the expected one"
                    .to_owned(),
            ))
        },
        }
    }
}
