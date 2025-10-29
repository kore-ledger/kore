

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
pub mod manual_distribution;
pub mod model;
pub mod node;
pub mod query;
pub mod request;
pub mod subject;
pub(crate) mod system;
pub mod update;
pub mod validation;

use approval::approver::ApprovalStateRes;
use auth::{Auth, AuthMessage, AuthResponse, AuthWitness};
use config::Config as KoreBaseConfig;
use error::Error;
use governance::Governance;
use helpers::db::ExternalDB;
use helpers::db::common::{
    ApproveInfo, EventInfo, PaginatorEvents, RequestInfo, SignaturesInfo,
    SubjectInfo,
};
use helpers::network::*;
use identity::identifier::DigestIdentifier;
use identity::identifier::derive::{KeyDerivator, digest::DigestDerivator};
use identity::keys::KeyPair;
use intermediary::Intermediary;
use manual_distribution::{ManualDistribution, ManualDistributionMessage};
use model::HashId;
use model::ValueWrapper;
use model::event::Event;
use model::signature::*;
use model::{SignTypesNode, request::*};
use network::{Monitor, MonitorMessage, MonitorResponse, NetworkWorker};
use rush::{ActorPath, ActorRef, Sink};
use tokio::sync::RwLock;

pub use network::MonitorNetworkState;

use node::register::{
    GovsData, Register, RegisterDataSubj, RegisterMessage, RegisterResponse,
};
use node::{Node, NodeMessage, NodeResponse, TransferSubject};
use prometheus_client::registry::Registry;
use query::{Query, QueryMessage, QueryResponse};
use request::{
    RequestData, RequestHandler, RequestHandlerMessage, RequestHandlerResponse,
};
use subject::{Subject, SubjectMessage, SubjectResponse};
use system::system;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use validation::{Validation, ValidationInfo, ValidationMessage};

use lazy_static::lazy_static;

use std::collections::HashMap;
use std::sync::Mutex;

use crate::config::SinkAuth;

#[cfg(all(feature = "sqlite", feature = "rocksdb"))]
compile_error!("Select only one: 'sqlite' or 'rocksdb'.");

#[cfg(not(any(feature = "sqlite", feature = "rocksdb")))]
compile_error!("You must enable 'sqlite' or 'rocksdb'.");

#[cfg(not(feature = "ext-sqlite"))]
compile_error!("You must enable 'ext-sqlite'.");

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

#[derive(Clone)]
pub struct Api {
    peer_id: String,
    controller_id: String,
    request: ActorRef<RequestHandler>,
    node: ActorRef<Node>,
    auth: ActorRef<Auth>,
    query: ActorRef<Query>,
    register: ActorRef<Register>,
    monitor: ActorRef<Monitor>,
    manual_dis: ActorRef<ManualDistribution>,
}

impl Api {
    /// Creates a new `Api`.
    pub async fn build(
        keys: KeyPair,
        config: KoreBaseConfig,
        sink_auth: SinkAuth,
        registry: &mut Registry,
        password: &str,
        token: &CancellationToken,
    ) -> Result<(Self, Vec<JoinHandle<()>>), Error> {
        info!(TARGET_API, "Creating Api");

        let (system, runner) =
            system(config.clone(), sink_auth, password, token.clone()).await?;

        let newtork_monitor = Monitor::default();
        let newtork_monitor_actor = system
            .create_root_actor("network_monitor", newtork_monitor)
            .await
            .map_err(|e| {
                error!(
                    TARGET_API,
                    "Can not create network_monitor actor {}", e
                );
                Error::System(e.to_string())
            })?;

        let mut worker: NetworkWorker<NetworkMessage> = NetworkWorker::new(
            registry,
            keys.clone(),
            config.network.clone(),
            Some(newtork_monitor_actor.clone()),
            config.key_derivator,
            token.clone(),
        )
        .map_err(|e| {
            error!(TARGET_API, "Can not create networt {}", e);
            Error::Network(e.to_string())
        })?;

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

        let worker_runner = tokio::spawn(async move {
            let _ = worker.run().await;
        });

        let node = Node::new(&keys)?;
        let node_actor =
            system.create_root_actor("node", node).await.map_err(|e| {
                let e = format!("Can not get node actor: {}", e);
                error!(TARGET_API, "Init system, {}", e);
                Error::System(e.to_owned())
            })?;

        let register: Option<ActorRef<Register>> = system
            .get_actor(&ActorPath::from("/user/node/register"))
            .await;
        let Some(register_actor) = register else {
            let e = "Can not get register actor";
            error!(TARGET_API, "Init system, {}", e);
            return Err(Error::System(e.to_owned()));
        };

        let manual_dis: Option<ActorRef<ManualDistribution>> = system
            .get_actor(&ActorPath::from("/user/node/manual_distribution"))
            .await;

        let Some(manual_dis_actor) = manual_dis else {
            let e = "Can not get manual_dis actor";
            error!(TARGET_API, "Init system, {}", e);
            return Err(Error::System(e.to_owned()));
        };

        let auth: Option<ActorRef<Auth>> =
            system.get_actor(&ActorPath::from("/user/node/auth")).await;
        let Some(auth_actor) = auth else {
            let e = "Can not get auth actor";
            error!(TARGET_API, "Init system, {}", e);
            return Err(Error::System(e.to_owned()));
        };

        let request = RequestHandler::new(keys.key_identifier());
        let request_actor = system
            .create_root_actor("request", request)
            .await
            .map_err(|e| {
                let e = format!("Can not get request actor: {}", e);
                error!(TARGET_API, "Init system, {}", e);
                Error::System(e.to_owned())
            })?;
        let Some(ext_db): Option<ExternalDB> =
            system.get_helper("ext_db").await
        else {
            let e = "Can not get ext_db helper";
            error!(TARGET_API, "Init system, {}", e);
            return Err(Error::System(e.to_owned()));
        };

        let sink =
            Sink::new(request_actor.subscribe(), ext_db.get_request_handler());
        system.run_sink(sink).await;

        let query = Query::new(keys.key_identifier());
        let query_actor = system
            .create_root_actor("query", query)
            .await
            .map_err(|e| {
                let e = format!("Can not get query actor: {}", e);
                error!(TARGET_API, "Init system, {}", e);
                Error::System(e.to_owned())
            })?;

        let tasks = Vec::from([runner, worker_runner]);

        Ok((
            Self {
                controller_id: keys.key_identifier().to_string(),
                peer_id,
                request: request_actor,
                auth: auth_actor,
                node: node_actor,
                query: query_actor,
                register: register_actor,
                monitor: newtork_monitor_actor,
                manual_dis: manual_dis_actor,
            },
            tasks,
        ))
    }

    pub fn peer_id(&self) -> String {
        self.peer_id.clone()
    }

    pub fn controller_id(&self) -> String {
        self.controller_id.clone()
    }

    pub async fn get_network_state(
        &self,
    ) -> Result<MonitorNetworkState, Error> {
        let response =
            self.monitor.ask(MonitorMessage::State).await.map_err(|e| {
                let e = format!("Can not get network state {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            MonitorResponse::State(state) => Ok(state),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    /// Request from issuer.
    pub async fn external_request(
        &self,
        request: Signed<EventRequest>,
    ) -> Result<RequestData, Error> {
        let response = self
            .request
            .ask(RequestHandlerMessage::NewRequest { request })
            .await
            .map_err(|e| {
                let e = format!("Can not send external request {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            RequestHandlerResponse::Ok(request_data) => Ok(request_data),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
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
            .await
            .map_err(|e| {
                let e =
                    format!("The node was unable to sign the request: {}", e);
                warn!(TARGET_API, e);
                Error::Node(e)
            })?;

        let signature = match response {
            NodeResponse::SignRequest(signature) => signature,
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                return Err(Error::Api(e.to_owned()));
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
                let e = format!("Can not send our request {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            RequestHandlerResponse::Ok(request_data) => Ok(request_data),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn get_pending_transfers(
        &self,
    ) -> Result<Vec<TransferSubject>, Error> {
        let response =
            self.node.ask(NodeMessage::PendingTransfers).await.map_err(
                |e| {
                    let e = format!(
                        "The node was unable to get pending transfers: {}",
                        e
                    );
                    warn!(TARGET_API, e);
                    Error::Node(e)
                },
            )?;

        let NodeResponse::PendingTransfers(pending) = response else {
            let e = "A response was received that was not the expected one";
            warn!(TARGET_API, e);
            return Err(Error::Api(e.to_owned()));
        };

        Ok(pending)
    }

    pub async fn request_state(
        &self,
        request_id: DigestIdentifier,
    ) -> Result<RequestInfo, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetRequestState {
                request_id: request_id.to_string(),
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get request state {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            QueryResponse::RequestState(state) => Ok(state),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn get_approval(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<ApproveInfo, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetApproval {
                subject_id: subject_id.to_string(),
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get approval request {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            QueryResponse::ApprovalState(data) => Ok(data),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn manual_distribution(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<String, Error> {
        self.manual_dis
            .ask(ManualDistributionMessage::Update(subject_id))
            .await
            .map_err(|e| {
                let e = format!("Can not post manual update {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        Ok("Manual update in progress".to_owned())
    }

    pub async fn approve(
        &self,
        subject_id: DigestIdentifier,
        state: ApprovalStateRes,
    ) -> Result<String, Error> {
        if let ApprovalStateRes::Obsolete = state {
            let e = "Invalid approval state";
            warn!(TARGET_API, e);
            return Err(Error::Api(e.to_owned()));
        }

        let response = self
            .request
            .ask(RequestHandlerMessage::ChangeApprovalState {
                subject_id: subject_id.to_string(),
                state,
            })
            .await
            .map_err(|e| {
                let e = format!("Can not change approve request state: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            RequestHandlerResponse::Response(res) => Ok(res),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn auth_subject(
        &self,
        subject_id: DigestIdentifier,
        witnesses: AuthWitness,
    ) -> Result<String, Error> {
        self.auth
            .tell(AuthMessage::NewAuth {
                subject_id,
                witness: witnesses,
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get auth subject: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        Ok("Ok".to_owned())
    }

    pub async fn all_auth_subjects(&self) -> Result<Vec<String>, Error> {
        let response =
            self.auth.ask(AuthMessage::GetAuths).await.map_err(|e| {
                error!(TARGET_API, "Can not get auth subject: {}", e);
                Error::Api(format!("Can not get auth subjects: {}", e))
            })?;

        match response {
            AuthResponse::Auths { subjects } => Ok(subjects),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn witnesses_subject(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<AuthWitness, Error> {
        let response = self
            .auth
            .ask(AuthMessage::GetAuth { subject_id })
            .await
            .map_err(|e| {
                let e = format!("Can not get witnesses of subjects: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            AuthResponse::Witnesses(witnesses) => Ok(witnesses),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn delete_auth_subject(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<String, Error> {
        self.auth
            .tell(AuthMessage::DeleteAuth { subject_id })
            .await
            .map_err(|e| {
                let e = format!("Can not delete auth of subjects: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        Ok("Ok".to_owned())
    }

    pub async fn check_transfer(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<String, Error> {
        let response = self
            .auth
            .ask(AuthMessage::CheckTransfer { subject_id })
            .await
            .map_err(|e| {
                let e = format!("Can not check Transfer: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            AuthResponse::None => Ok("Checking in progress".to_owned()),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn update_subject(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<String, Error> {
        let response = self
            .auth
            .ask(AuthMessage::Update {
                subject_id,
                more_info: auth::WitnessesAuth::None,
            })
            .await
            .map_err(|e| {
                let e = format!("Can not update subject: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            AuthResponse::None => Ok("Update in progress".to_owned()),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn all_govs(
        &self,
        active: Option<bool>,
    ) -> Result<Vec<GovsData>, Error> {
        let response = self
            .register
            .ask(RegisterMessage::GetGovs { active })
            .await
            .map_err(|e| {
                let e = format!("Can not get resgister governances: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            RegisterResponse::Govs { governances } => Ok(governances),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn all_subjs(
        &self,
        gov_id: DigestIdentifier,
        active: Option<bool>,
        schema_id: Option<String>,
    ) -> Result<Vec<RegisterDataSubj>, Error> {
        let response = self
            .register
            .ask(RegisterMessage::GetSubj {
                gov_id: gov_id.to_string(),
                active,
                schema_id,
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get resgister subjects: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            RegisterResponse::Subjs { subjects } => Ok(subjects),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn get_events(
        &self,
        subject_id: DigestIdentifier,
        quantity: Option<u64>,
        page: Option<u64>,
        reverese: Option<bool>,
    ) -> Result<PaginatorEvents, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetEvents {
                subject_id: subject_id.to_string(),
                quantity,
                page,
                reverese,
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get events: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            QueryResponse::PaginatorEvents(data) => Ok(data),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn get_event_sn(
        &self,
        subject_id: DigestIdentifier,
        sn: u64,
    ) -> Result<EventInfo, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetEventSn {
                subject_id: subject_id.to_string(),
                sn,
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get event sn: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            QueryResponse::Event(data) => Ok(data),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn get_first_or_end_events(
        &self,
        subject_id: DigestIdentifier,
        quantity: Option<u64>,
        reverse: Option<bool>,
        success: Option<bool>,
    ) -> Result<Vec<EventInfo>, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetFirstOrEndEvents {
                subject_id: subject_id.to_string(),
                quantity,
                reverse,
                success,
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get first or end events: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            QueryResponse::Events(data) => Ok(data),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn get_subject(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<SubjectInfo, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetSubject {
                subject_id: subject_id.to_string(),
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get subject: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            QueryResponse::Subject(subject) => Ok(subject),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }

    pub async fn get_signatures(
        &self,
        subject_id: DigestIdentifier,
    ) -> Result<SignaturesInfo, Error> {
        let response = self
            .query
            .ask(QueryMessage::GetSignatures {
                subject_id: subject_id.to_string(),
            })
            .await
            .map_err(|e| {
                let e = format!("Can not get signatures: {}", e);
                warn!(TARGET_API, e);
                Error::Api(e)
            })?;

        match response {
            QueryResponse::Signatures(signatures) => Ok(signatures),
            _ => {
                let e = "A response was received that was not the expected one";
                warn!(TARGET_API, e);
                Err(Error::Api(e.to_owned()))
            }
        }
    }
}
