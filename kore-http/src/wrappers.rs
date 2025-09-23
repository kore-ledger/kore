use kore_bridge::{
    ApprovalReqInfo as ApprovalReqInfoBridge, ApproveInfo as ApproveInfoBridge,
    ConfirmRequestInfo as ConfirmRequestInfoBridge,
    ControlListConfig as ControlListConfigBridge,
    CreateRequestInfo as CreateRequestInfoBridge,
    EOLRequestInfo as EOLRequestInfoBridge, EventInfo as EventInfoBridge,
    EventRequestInfo as EventRequestInfoBridge, FactInfo as FactInfoBridge,
    FactRequestInfo as FactRequestInfoBridge, GovsData as GovsDataBridge,
    KoreConfig as KoreConfigBridge, Logging as LoggingBridge,
    LoggingOutput as LoggingOutputBridge,
    LoggingRotation as LoggingRotationBridge, Namespace as NamespaceBridge,
    NetworkConfig as NetworkConfigBridge, Paginator as PaginatorBridge,
    PaginatorEvents as PaginatorEventsBridge,
    ProtocolsError as ProtocolsErrorBridge,
    ProtocolsSignaturesInfo as ProtocolsSignaturesInfoBridge,
    RegisterDataSubj as RegisterDataSubjBridge,
    RejectRequestInfo as RejectRequestInfoBridge,
    RequestData as RequestDataBridge, RequestInfo as RequestInfoBridge,
    RoutingConfig as RoutingConfigBridge, RoutingNode as RoutingNodeBridge,
    SignatureInfo as SignatureInfoBridge,
    SignaturesInfo as SignaturesInfoBridge, SignedInfo as SignedInfoBridge,
    SinkConfig as SinkConfigBridge, SubjectInfo as SubjectInfoBridge,
    TellConfig as TellConfigBridge,
    TimeOutResponseInfo as TimeOutResponseInfoBridge,
    TransferRequestInfo as TransferRequestInfoBridge,
    TransferSubject as TransferSubjectBridge, config::Config as ConfigBridge,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TransferSubject {
    pub name: String,
    pub subject_id: String,
    pub new_owner: String,
    pub actual_owner: String,
}

impl From<TransferSubjectBridge> for TransferSubject {
    fn from(value: TransferSubjectBridge) -> Self {
        Self {
            name: value.name,
            actual_owner: value.actual_owner,
            new_owner: value.new_owner,
            subject_id: value.subject_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginatorEvents {
    pub paginator: Paginator,
    pub events: Vec<EventInfo>,
}

impl From<PaginatorEventsBridge> for PaginatorEvents {
    fn from(value: PaginatorEventsBridge) -> Self {
        Self {
            paginator: Paginator::from(value.paginator),
            events: value
                .events
                .iter()
                .map(|x| EventInfo::from(x.clone()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EventInfo {
    pub subject_id: String,
    pub sn: u64,
    pub patch: Option<Value>,
    pub error: Option<ProtocolsError>,
    pub event_req: EventRequestInfo,
    pub succes: bool,
}

impl From<EventInfoBridge> for EventInfo {
    fn from(value: EventInfoBridge) -> Self {
        let error = value.error.map(ProtocolsError::from);

        Self {
            subject_id: value.subject_id,
            sn: value.sn,
            patch: value.patch,
            error,
            event_req: EventRequestInfo::from(value.event_req),
            succes: value.succes,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Paginator {
    pub pages: u64,
    pub next: Option<u64>,
    pub prev: Option<u64>,
}

impl From<PaginatorBridge> for Paginator {
    fn from(value: PaginatorBridge) -> Self {
        Self {
            pages: value.pages,
            next: value.next,
            prev: value.prev,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProtocolsError {
    pub evaluation: Option<String>,
    pub validation: Option<String>,
}

impl From<ProtocolsErrorBridge> for ProtocolsError {
    fn from(value: ProtocolsErrorBridge) -> Self {
        Self {
            evaluation: value.evaluation,
            validation: value.validation,
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Serialize, ToSchema)]
pub enum EventRequestInfo {
    Create(CreateRequestInfo),
    Fact(FactRequestInfo),
    Transfer(TransferRequestInfo),
    Confirm(ConfirmRequestInfo),
    Reject(RejectRequestInfo),
    EOL(EOLRequestInfo),
}

impl From<EventRequestInfoBridge> for EventRequestInfo {
    fn from(value: EventRequestInfoBridge) -> Self {
        match value {
            EventRequestInfoBridge::Create(create_request_info) => {
                Self::Create(CreateRequestInfo::from(create_request_info))
            }
            EventRequestInfoBridge::Fact(fact_request_info) => {
                Self::Fact(FactRequestInfo::from(fact_request_info))
            }
            EventRequestInfoBridge::Transfer(transfer_request_info) => {
                Self::Transfer(TransferRequestInfo::from(transfer_request_info))
            }
            EventRequestInfoBridge::Confirm(confirm_request_info) => {
                Self::Confirm(ConfirmRequestInfo::from(confirm_request_info))
            }
            EventRequestInfoBridge::Reject(reject_request_info) => {
                Self::Reject(RejectRequestInfo::from(reject_request_info))
            }
            EventRequestInfoBridge::EOL(eolrequest_info) => {
                Self::EOL(EOLRequestInfo::from(eolrequest_info))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CreateRequestInfo {
    pub governance_id: String,
    pub schema_id: String,
    pub namespace: Namespace,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl From<CreateRequestInfoBridge> for CreateRequestInfo {
    fn from(value: CreateRequestInfoBridge) -> Self {
        Self {
            governance_id: value.governance_id,
            schema_id: value.schema_id,
            namespace: Namespace::from(value.namespace),
            description: value.description,
            name: value.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TransferRequestInfo {
    pub subject_id: String,
    pub new_owner: String,
}

impl From<TransferRequestInfoBridge> for TransferRequestInfo {
    fn from(value: TransferRequestInfoBridge) -> Self {
        Self {
            subject_id: value.subject_id,
            new_owner: value.new_owner,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConfirmRequestInfo {
    pub subject_id: String,
    pub name_old_owner: Option<String>,
}

impl From<ConfirmRequestInfoBridge> for ConfirmRequestInfo {
    fn from(value: ConfirmRequestInfoBridge) -> Self {
        Self {
            subject_id: value.subject_id,
            name_old_owner: value.name_old_owner,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RejectRequestInfo {
    pub subject_id: String,
}

impl From<RejectRequestInfoBridge> for RejectRequestInfo {
    fn from(value: RejectRequestInfoBridge) -> Self {
        Self {
            subject_id: value.subject_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EOLRequestInfo {
    pub subject_id: String,
}

impl From<EOLRequestInfoBridge> for EOLRequestInfo {
    fn from(value: EOLRequestInfoBridge) -> Self {
        Self {
            subject_id: value.subject_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FactRequestInfo {
    pub subject_id: String,
    pub payload: Value,
}

impl From<FactRequestInfoBridge> for FactRequestInfo {
    fn from(value: FactRequestInfoBridge) -> Self {
        Self {
            subject_id: value.subject_id,
            payload: value.payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Namespace(Vec<String>);

impl From<NamespaceBridge> for Namespace {
    fn from(value: NamespaceBridge) -> Self {
        Namespace::from(value.to_string())
    }
}

impl From<&str> for Namespace {
    fn from(str: &str) -> Self {
        let tokens: Vec<String> = str
            .split('.')
            .filter(|x| !x.trim().is_empty())
            .map(|s| s.to_string())
            .collect();

        Namespace(tokens)
    }
}

impl From<String> for Namespace {
    fn from(str: String) -> Self {
        Namespace::from(str.as_str())
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RequestData {
    pub request_id: String,
    pub subject_id: String,
}

impl From<RequestDataBridge> for RequestData {
    fn from(value: RequestDataBridge) -> Self {
        Self {
            request_id: value.request_id,
            subject_id: value.subject_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GovsData {
    pub governance_id: String,
    pub active: bool,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl From<GovsDataBridge> for GovsData {
    fn from(value: GovsDataBridge) -> Self {
        Self {
            governance_id: value.governance_id,
            active: value.active,
            description: value.description,
            name: value.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RegisterDataSubj {
    pub subject_id: String,
    pub schema_id: String,
    pub active: bool,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl From<RegisterDataSubjBridge> for RegisterDataSubj {
    fn from(value: RegisterDataSubjBridge) -> Self {
        Self {
            subject_id: value.subject_id,
            schema_id: value.schema_id,
            active: value.active,
            name: value.name,
            description: value.description,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RequestInfo {
    status: String,
    version: u64,
    error: Option<String>,
}

impl From<RequestInfoBridge> for RequestInfo {
    fn from(value: RequestInfoBridge) -> Self {
        Self {
            status: value.status,
            version: value.version,
            error: value.error,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ApproveInfo {
    pub state: String,
    pub request: ApprovalReqInfo,
}

impl From<ApproveInfoBridge> for ApproveInfo {
    fn from(value: ApproveInfoBridge) -> Self {
        Self {
            state: value.state,
            request: ApprovalReqInfo::from(value.request),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
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

impl From<ApprovalReqInfoBridge> for ApprovalReqInfo {
    fn from(value: ApprovalReqInfoBridge) -> Self {
        Self {
            event_request: SignedInfo::from(value.event_request),
            sn: value.sn,
            gov_version: value.gov_version,
            patch: value.patch,
            state_hash: value.state_hash,
            hash_prev_event: value.hash_prev_event,
            subject_id: value.subject_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SignedInfo<T>
where
    T: Serialize + Clone,
{
    pub content: T,
    pub signature: SignatureInfo,
}

impl From<SignedInfoBridge<FactInfoBridge>> for SignedInfo<FactInfo> {
    fn from(value: SignedInfoBridge<FactInfoBridge>) -> Self {
        Self {
            content: FactInfo::from(value.content),
            signature: SignatureInfo::from(value.signature),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FactInfo {
    payload: Value,
    subject_id: String,
}

impl From<FactInfoBridge> for FactInfo {
    fn from(value: FactInfoBridge) -> Self {
        Self {
            payload: value.payload,
            subject_id: value.subject_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq, Hash)]
pub struct SignatureInfo {
    pub signer: String,
    pub timestamp: u64,
    pub content_hash: String,
    pub value: String,
}

impl From<SignatureInfoBridge> for SignatureInfo {
    fn from(value: SignatureInfoBridge) -> Self {
        Self {
            signer: value.signer,
            timestamp: value.timestamp,
            content_hash: value.content_hash,
            value: value.value,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SubjectInfo {
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
    pub name: String,
    pub description: String,
}

impl From<SubjectInfoBridge> for SubjectInfo {
    fn from(value: SubjectInfoBridge) -> Self {
        Self {
            subject_id: value.subject_id,
            governance_id: value.governance_id,
            genesis_gov_version: value.genesis_gov_version,
            namespace: value.namespace,
            schema_id: value.schema_id,
            owner: value.owner,
            creator: value.creator,
            active: value.active,
            sn: value.sn,
            properties: value.properties,
            new_owner: value.new_owner,
            description: value.description,
            name: value.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SignaturesInfo {
    pub subject_id: String,
    pub sn: u64,
    pub signatures_eval: Option<HashSet<ProtocolsSignaturesInfo>>,
    pub signatures_appr: Option<HashSet<ProtocolsSignaturesInfo>>,
    pub signatures_vali: HashSet<ProtocolsSignaturesInfo>,
}

impl From<SignaturesInfoBridge> for SignaturesInfo {
    fn from(value: SignaturesInfoBridge) -> Self {
        let signatures_eval = value.signatures_eval.map(|eval| {
            eval.iter()
                .map(|x| ProtocolsSignaturesInfo::from(x.clone()))
                .collect()
        });

        let signatures_appr = value.signatures_appr.map(|appr| {
            appr.iter()
                .map(|x| ProtocolsSignaturesInfo::from(x.clone()))
                .collect()
        });

        let signatures_vali = value
            .signatures_vali
            .iter()
            .map(|x| ProtocolsSignaturesInfo::from(x.clone()))
            .collect();

        Self {
            subject_id: value.subject_id,
            sn: value.sn,
            signatures_eval,
            signatures_appr,
            signatures_vali,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq, Hash)]
pub enum ProtocolsSignaturesInfo {
    Signature(SignatureInfo),
    TimeOut(TimeOutResponseInfo),
}

impl From<ProtocolsSignaturesInfoBridge> for ProtocolsSignaturesInfo {
    fn from(value: ProtocolsSignaturesInfoBridge) -> Self {
        match value {
            ProtocolsSignaturesInfoBridge::Signature(signature_info) => {
                Self::Signature(SignatureInfo::from(signature_info))
            }
            ProtocolsSignaturesInfoBridge::TimeOut(time_out_response_info) => {
                Self::TimeOut(TimeOutResponseInfo::from(time_out_response_info))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq, Hash)]
pub struct TimeOutResponseInfo {
    pub who: String,
    pub re_trys: u32,
    pub timestamp: u64,
}

impl From<TimeOutResponseInfoBridge> for TimeOutResponseInfo {
    fn from(value: TimeOutResponseInfoBridge) -> Self {
        Self {
            who: value.who,
            re_trys: value.re_trys,
            timestamp: value.timestamp,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct Config {
    pub kore_config: KoreConfig,
    pub keys_path: String,
    pub prometheus: String,
    pub logging: Logging,
    pub sink: SinkConfig,
}

impl From<ConfigBridge> for Config {
    fn from(value: ConfigBridge) -> Self {
        Self {
            kore_config: KoreConfig::from(value.kore_config),
            keys_path: value.keys_path,
            prometheus: value.prometheus,
            logging: Logging::from(value.logging),
            sink: SinkConfig::from(value.sink),
        }
    }
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct LoggingOutput {
    pub stdout: bool,
    pub file: bool,
    pub api: bool,
}

impl From<LoggingOutputBridge> for LoggingOutput {
    fn from(value: LoggingOutputBridge) -> Self {
        Self {
            stdout: value.stdout,
            file: value.file,
            api: value.api,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LoggingRotation {
    Size,
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Never,
}

impl From<LoggingRotationBridge> for LoggingRotation {
    fn from(value: LoggingRotationBridge) -> Self {
        match value {
            LoggingRotationBridge::Size => LoggingRotation::Size,
            LoggingRotationBridge::Hourly => LoggingRotation::Hourly,
            LoggingRotationBridge::Daily => LoggingRotation::Daily,
            LoggingRotationBridge::Weekly => LoggingRotation::Weekly,
            LoggingRotationBridge::Monthly => LoggingRotation::Monthly,
            LoggingRotationBridge::Yearly => LoggingRotation::Yearly,
            LoggingRotationBridge::Never => LoggingRotation::Never,
        }
    }
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct Logging {
    pub output: LoggingOutput,
    pub api_url: Option<String>,
    pub file_path: String,
    pub rotation: LoggingRotation,
    pub max_size: usize,
    pub max_files: usize,
}

impl From<LoggingBridge> for Logging {
    fn from(value: LoggingBridge) -> Self {
        Self {
            output: LoggingOutput::from(value.output),
            api_url: value.api_url,
            file_path: value.file_path,
            rotation: LoggingRotation::from(value.rotation),
            max_size: value.max_size,
            max_files: value.max_files,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SinkConfig {
    pub sinks: String,
    pub auth: String,
    pub username: String,
}

impl From<SinkConfigBridge> for SinkConfig {
    fn from(value: SinkConfigBridge) -> Self {
        let mut sinks = String::default();
        for (schema_id, val) in value.sinks.iter() {
            for sink_server in val {
                let mut event_string = String::default();
                for event in sink_server.events.iter() {
                    event_string = format!("{} ", event);
                }

                if !event_string.is_empty() {
                    event_string.remove(0);
                }

                sinks = format!(
                    ",{}|{}|{}|{}|{}",
                    sink_server.server,
                    schema_id,
                    event_string,
                    sink_server.url,
                    sink_server.auth
                );
            }
        }

        if !sinks.is_empty() {
            sinks.remove(0);
        }

        Self {
            sinks,
            auth: value.auth,
            username: value.username,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct KoreConfig {
    pub key_derivator: String,
    pub digest_derivator: String,
    pub kore_db: String,
    pub external_db: String,
    pub network: NetworkConfig,
    pub contracts_dir: String,
    pub always_accept: bool,
    pub garbage_collector: u64,
}

impl From<KoreConfigBridge> for KoreConfig {
    fn from(value: KoreConfigBridge) -> Self {
        Self {
            key_derivator: value.key_derivator.to_string(),
            digest_derivator: value.digest_derivator.to_string(),
            kore_db: value.kore_db.to_string(),
            external_db: value.external_db.to_string(),
            network: NetworkConfig::from(value.network),
            contracts_dir: value.contracts_dir,
            always_accept: value.always_accept,
            garbage_collector: value.garbage_collector.as_secs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NetworkConfig {
    pub node_type: String,
    pub boot_nodes: Vec<RoutingNode>,
    pub listen_addresses: Vec<String>,
    pub external_addresses: Vec<String>,
    pub tell: TellConfig,
    pub routing: RoutingConfig,
    pub control_list: ControlListConfig,
}

impl From<NetworkConfigBridge> for NetworkConfig {
    fn from(value: NetworkConfigBridge) -> Self {
        Self {
            boot_nodes: value
                .boot_nodes
                .iter()
                .map(|x| RoutingNode::from(x.clone()))
                .collect(),
            node_type: value.node_type.to_string(),
            listen_addresses: value.listen_addresses,
            external_addresses: value.external_addresses,
            tell: TellConfig::from(value.tell),
            routing: RoutingConfig::from(value.routing),
            control_list: ControlListConfig::from(value.control_list),
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ControlListConfig {
    enable: bool,
    allow_list: Vec<String>,
    block_list: Vec<String>,
    service_allow_list: Vec<String>,
    service_block_list: Vec<String>,
    interval_request: u64,
}

impl From<ControlListConfigBridge> for ControlListConfig {
    fn from(value: ControlListConfigBridge) -> Self {
        Self {
            enable: value.get_enable(),
            allow_list: value.get_allow_list(),
            block_list: value.get_block_list(),
            service_allow_list: value.get_service_allow_list(),
            service_block_list: value.get_service_block_list(),
            interval_request: value.get_interval_request().as_secs(),
        }
    }
}
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TellConfig {
    pub message_timeout: u64,
    pub max_concurrent_streams: usize,
}

impl From<TellConfigBridge> for TellConfig {
    fn from(value: TellConfigBridge) -> Self {
        Self {
            message_timeout: value.message_timeout.as_secs(),
            max_concurrent_streams: value.max_concurrent_streams,
        }
    }
}
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct RoutingConfig {
    dht_random_walk: bool,
    discovery_only_if_under_num: u64,
    allow_private_address_in_dht: bool,
    allow_dns_address_in_dht: bool,
    allow_loop_back_address_in_dht: bool,
    kademlia_disjoint_query_paths: bool,
}

impl From<RoutingConfigBridge> for RoutingConfig {
    fn from(value: RoutingConfigBridge) -> Self {
        Self {
            dht_random_walk: value.get_dht_random_walk(),
            discovery_only_if_under_num: value.get_discovery_limit(),
            allow_private_address_in_dht: value.get_allow_private_address_in_dht(),
            allow_dns_address_in_dht: value.get_allow_dns_address_in_dht(),
            allow_loop_back_address_in_dht: value
                .get_allow_loop_back_address_in_dht(),
            kademlia_disjoint_query_paths: value
                .get_kademlia_disjoint_query_paths(),
        }
    }
}
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct RoutingNode {
    pub peer_id: String,
    pub address: Vec<String>,
}

impl From<RoutingNodeBridge> for RoutingNode {
    fn from(value: RoutingNodeBridge) -> Self {
        Self {
            peer_id: value.peer_id,
            address: value.address,
        }
    }
}
