use std::collections::{HashMap, HashSet};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler,
    SystemEvent,
};
use borsh::{BorshDeserialize, BorshSerialize};
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use wasmtime::{Caller, Engine, Linker};

use crate::{
    auth::{Auth, AuthMessage},
    governance::{model::Roles, Quorum},
    intermediary::Intermediary,
    model::SignTypesNode,
    node::relationship::{
        OwnerSchema, RelationShip, RelationShipMessage, RelationShipResponse,
    },
    request::manager::{RequestManager, RequestManagerMessage},
    subject::{
        event::{LedgerEvent, LedgerEventMessage, LedgerEventResponse},
        Metadata,
    },
    ActorMessage, Error, Event as KoreEvent, EventRequestType, Governance,
    NetworkMessage, Node, NodeMessage, NodeResponse, Signature, Signed,
    Subject, SubjectMessage, SubjectResponse, SubjectsTypes,
};
use tracing::error;

use super::{event::ProtocolsSignatures, HashId, Namespace};

const TARGET_COMMON: &str = "Kore-Model-Common";

#[derive(Debug, Default)]
pub struct MemoryManager {
    memory: Vec<u8>,
    map: HashMap<usize, usize>,
}

impl MemoryManager {
    pub fn alloc(&mut self, len: usize) -> usize {
        let current_len = self.memory.len();
        self.memory.resize(current_len + len, 0);
        self.map.insert(current_len, len);
        current_len
    }

    pub fn write_byte(&mut self, start_ptr: usize, offset: usize, data: u8) {
        self.memory[start_ptr + offset] = data;
    }

    pub fn read_byte(&self, ptr: usize) -> u8 {
        self.memory[ptr]
    }

    pub fn read_data(&self, ptr: usize) -> Result<&[u8], Error> {
        let len = self
            .map
            .get(&ptr)
            .ok_or(Error::Runner("Invalid pointer provided".to_owned()))?;
        Ok(&self.memory[ptr..ptr + len])
    }

    pub fn get_pointer_len(&self, ptr: usize) -> isize {
        let Some(result) = self.map.get(&ptr) else {
            return -1;
        };
        *result as isize
    }

    pub fn add_data_raw(&mut self, bytes: &[u8]) -> usize {
        let ptr = self.alloc(bytes.len());
        for (index, byte) in bytes.iter().enumerate() {
            self.memory[ptr + index] = *byte;
        }
        ptr
    }
}

pub async fn get_gov<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<Governance, ActorError>
where
    A: Actor + Handler<A>,
{
    // Subject path
    let subject_path = ActorPath::from(format!("/user/node/{}", subject_id));

    // Subject actor.
    let subject_actor: Option<ActorRef<Subject>> =
        ctx.system().get_actor(&subject_path).await;

    // We obtain the actor governance
    let response = if let Some(subject_actor) = subject_actor {
        subject_actor.ask(SubjectMessage::GetGovernance).await?
    } else {
        return Err(ActorError::NotFound(subject_path));
    };

    match response {
        SubjectResponse::Governance(gov) => Ok(gov),
        _ => Err(ActorError::UnexpectedResponse(
            subject_path,
            "SubjectResponse::Governance".to_owned(),
        )),
    }
}

pub async fn get_metadata<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<Metadata, ActorError>
where
    A: Actor + Handler<A>,
{
    let subject_path = ActorPath::from(format!("/user/node/{}", subject_id));
    let subject_actor: Option<ActorRef<Subject>> =
        ctx.system().get_actor(&subject_path).await;

    let response = if let Some(subject_actor) = subject_actor {
        subject_actor.ask(SubjectMessage::GetMetadata).await?
    } else {
        return Err(ActorError::NotFound(subject_path));
    };

    match response {
        SubjectResponse::Metadata(metadata) => Ok(metadata),
        _ => Err(ActorError::UnexpectedResponse(
            subject_path,
            "SubjectResponse::Metadata".to_owned(),
        )),
    }
}

pub async fn get_sign<A>(
    ctx: &mut ActorContext<A>,
    sign_type: SignTypesNode,
) -> Result<Signature, ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    // We obtain the validator
    let node_response = if let Some(node_actor) = node_actor {
        node_actor.ask(NodeMessage::SignRequest(sign_type)).await?
    } else {
        return Err(ActorError::NotFound(node_path));
    };

    match node_response {
        NodeResponse::SignRequest(signature) => Ok(signature),
        _ => Err(ActorError::UnexpectedResponse(
            node_path,
            "NodeResponse::SignRequest".to_owned(),
        )),
    }
}

pub async fn update_event<A>(
    ctx: &mut ActorContext<A>,
    event: Signed<KoreEvent>,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let ledger_event_path = ActorPath::from(format!(
        "/user/node/{}/ledger_event",
        event.content.subject_id
    ));
    let ledger_event_actor: Option<ActorRef<LedgerEvent>> =
        ctx.system().get_actor(&ledger_event_path).await;

    let response = if let Some(ledger_event_actor) = ledger_event_actor {
        ledger_event_actor
            .ask(LedgerEventMessage::UpdateLastEvent { event })
            .await?
    } else {
        return Err(ActorError::NotFound(ledger_event_path));
    };

    if let LedgerEventResponse::LastEvent(_) = response {
        return Err(ActorError::UnexpectedResponse(
            ledger_event_path,
            "LedgerEventResponse::Ok".to_owned(),
        ));
    }

    Ok(())
}

pub async fn change_temp_subj<A>(
    ctx: &mut ActorContext<A>,
    subject_id: String,
    key_identifier: String,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    if let Some(node_actor) = node_actor {
        node_actor
            .tell(NodeMessage::RegisterSubject(SubjectsTypes::ChangeTemp {
                subject_id,
                key_identifier,
            }))
            .await?;
    } else {
        return Err(ActorError::NotFound(node_path));
    }
    Ok(())
}

pub async fn get_quantity<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
    owner: String,
    namespace: String,
) -> Result<usize, ActorError>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        relation_actor
            .ask(RelationShipMessage::GetSubjectsCount(OwnerSchema {
                owner,
                gov,
                schema,
                namespace,
            }))
            .await?
    } else {
        return Err(ActorError::NotFound(relation_path));
    };

    if let RelationShipResponse::Count(quantity) = response {
        Ok(quantity)
    } else {
        Err(ActorError::UnexpectedResponse(
            relation_path,
            "RelationShipResponse::Count".to_owned(),
        ))
    }
}

pub async fn register_relation<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
    owner: String,
    subject: String,
    namespace: String,
    max_quantity: usize,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        relation_actor
            .ask(RelationShipMessage::RegisterNewSubject {
                data: OwnerSchema {
                    owner,
                    gov,
                    schema,
                    namespace,
                },
                subject,
                max_quantity,
            })
            .await?
    } else {
        return Err(ActorError::NotFound(relation_path));
    };

    match response {
        RelationShipResponse::None => Ok(()),
        _ => Err(ActorError::UnexpectedResponse(
            relation_path,
            "RelationShipResponse::None".to_owned(),
        )),
    }
}

pub async fn delete_relation<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema: String,
    owner: String,
    subject: String,
    namespace: String,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let relation_path = ActorPath::from("/user/node/relation_ship");
    let relation_actor: Option<ActorRef<RelationShip>> =
        ctx.system().get_actor(&relation_path).await;

    let response = if let Some(relation_actor) = relation_actor {
        relation_actor
            .ask(RelationShipMessage::DeleteSubject {
                data: OwnerSchema {
                    owner,
                    gov,
                    schema,
                    namespace,
                },
                subject,
            })
            .await?
    } else {
        return Err(ActorError::NotFound(relation_path));
    };

    if let RelationShipResponse::None = response {
        Ok(())
    } else {
        Err(ActorError::UnexpectedResponse(
            relation_path,
            "RelationShipResponse::None".to_owned(),
        ))
    }
}

pub fn verify_protocols_state(
    request: EventRequestType,
    eval: Option<bool>,
    approve: Option<bool>,
    approval_require: bool,
    val: bool,
) -> Result<bool, Error> {
    match request {
        EventRequestType::Create
        | EventRequestType::Transfer
        | EventRequestType::Confirm
        | EventRequestType::EOL => {
            if approve.is_some() || eval.is_some() || approval_require {
                return Err(Error::Protocols("In create, transferm, confirm and eol request, approve and eval must be None and approval require must be false".to_owned()));
            }
            Ok(val)
        }
        EventRequestType::Fact => {
            let Some(eval) = eval else {
                return Err(Error::Protocols(
                    "In Fact even eval must be Some".to_owned(),
                ));
            };

            if approval_require {
                let Some(approve) = approve else {
                    return Err(Error::Protocols("In Fact even if approval was required, approve must be Some".to_owned()));
                };
                Ok(eval && approve && val)
            } else {
                if approve.is_some() {
                    return Err(Error::Protocols("In Fact even if approval was not required, approve must be None".to_owned()));
                }

                Ok(val && eval)
            }
        }
    }
}

pub async fn get_signers_quorum_gov_version<A>(
    ctx: &mut ActorContext<A>,
    governance: &str,
    schema_id: &str,
    namespace: Namespace,
    role: Roles,
) -> Result<(HashSet<KeyIdentifier>, Quorum, u64), ActorError>
where
    A: Actor + Handler<A>,
{
    let gov = get_gov(ctx, governance).await?;
    let (signers, quorum) =
        gov.get_quorum_and_signers(role, schema_id, namespace)?;
    Ok((signers, quorum, gov.version))
}

pub async fn emit_fail<A>(
    ctx: &mut ActorContext<A>,
    error: ActorError,
) -> ActorError
where
    A: Actor + Handler<A>,
{
    if let Err(e) = ctx.emit_fail(error.clone()).await {
        error!(TARGET_COMMON, "EmitFail, can not emit fail: {}", e);
        ctx.system().send_event(SystemEvent::StopSystem).await;
    };
    error
}

pub async fn try_to_update_subject<A>(
    ctx: &mut ActorContext<A>,
    subject_id: DigestIdentifier,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let auth_path = ActorPath::from("/user/node/auth");
    let auth_actor: Option<ActorRef<Auth>> =
        ctx.system().get_actor(&auth_path).await;

    if let Some(auth_actor) = auth_actor {
        auth_actor.tell(AuthMessage::Update { subject_id }).await?;
    } else {
        return Err(ActorError::NotFound(auth_path));
    }

    Ok(())
}

pub fn generate_linker(
    engine: &Engine,
) -> Result<Linker<MemoryManager>, Error> {
    let mut linker: Linker<MemoryManager> = Linker::new(engine);

    // functions are created for webasembly modules, the logic of which is programmed in Rust
    linker
        .func_wrap(
            "env",
            "pointer_len",
            |caller: Caller<'_, MemoryManager>, pointer: i32| {
                return caller.data().get_pointer_len(pointer as usize)
                    as u32;
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: pointer_len, {}", e))
        })?;

    linker
        .func_wrap(
            "env",
            "alloc",
            |mut caller: Caller<'_, MemoryManager>, len: u32| {
                return caller.data_mut().alloc(len as usize) as u32;
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: allow, {}", e))
        })?;

    linker
        .func_wrap(
            "env",
            "write_byte",
            |mut caller: Caller<'_, MemoryManager>, ptr: u32, offset: u32, data: u32| {
                return caller
                    .data_mut()
                    .write_byte(ptr as usize, offset as usize, data as u8);
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: write_byte, {}", e))
        })?;

    linker
        .func_wrap(
            "env",
            "read_byte",
            |caller: Caller<'_, MemoryManager>, index: i32| {
                return caller.data().read_byte(index as usize) as u32;
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: read_byte, {}", e))
        })?;

    linker
        .func_wrap(
            "env",
            "cout",
            |_caller: Caller<'_, MemoryManager>, ptr: u32| {
                error!(TARGET_COMMON, "{}", ptr);
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: cout, {}", e))
        })?;
    Ok(linker)
}

pub async fn try_to_update<A>(
    vec: Vec<ProtocolsSignatures>,
    ctx: &mut ActorContext<A>,
    subject_id: DigestIdentifier,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let all_time_out = vec
        .iter()
        .all(|x| matches!(x, ProtocolsSignatures::TimeOut(_)));

    if all_time_out {
        let auth_path = ActorPath::from("/user/node/auth");
        let auth_actor: Option<ActorRef<Auth>> =
            ctx.system().get_actor(&auth_path).await;

        if let Some(auth_actor) = auth_actor {
            auth_actor.tell(AuthMessage::Update { subject_id }).await?;
        } else {
            return Err(ActorError::NotFound(auth_path));
        }
    }
    Ok(())
}

pub async fn check_request_owner<A, T>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
    owner: &str,
    req: Signed<T>,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
    T: BorshSerialize + BorshDeserialize + Clone + HashId,
{
    // Aquí hay que comprobar que el owner del subject es el que envía la req.
    let subject_path = ActorPath::from(format!("/user/node/{}", subject_id));
    let subject_actor: Option<ActorRef<Subject>> =
        ctx.system().get_actor(&subject_path).await;

    // We obtain the evaluator
    let response = if let Some(subject_actor) = subject_actor {
        subject_actor.ask(SubjectMessage::GetOwner).await?
    } else {
        return Err(ActorError::NotFound(subject_path));
    };

    let subject_owner = match response {
        SubjectResponse::Owner(owner) => owner,
        _ => {
            return Err(ActorError::UnexpectedResponse(
                subject_path,
                "SubjectResponse::Owner".to_owned(),
            ))
        }
    };

    if subject_owner.to_string() != owner {
        // TODO ver si tengo la última version de la gov.
        return Err(ActorError::Functional(
            "Evaluation req signer and owner are not the same".to_owned(),
        ));
    }

    if let Err(e) = req.verify() {
        return Err(ActorError::Functional(format!(
            "Can not verify signature: {}",
            e
        )));
    }

    Ok(())
}

pub struct UpdateData {
    pub sn: u64,
    pub gov_version: u64,
    pub subject_id: DigestIdentifier,
    pub our_node: KeyIdentifier,
    pub other_node: KeyIdentifier,
}

pub async fn update_ledger_network<A>(
    ctx: &mut ActorContext<A>,
    data: UpdateData,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let subject_string = data.subject_id.to_string();
    let request = ActorMessage::DistributionLedgerReq {
        gov_version: Some(data.gov_version),
        actual_sn: Some(data.sn),
        subject_id: data.subject_id,
    };

    let info = ComunicateInfo {
        reciver: data.other_node,
        sender: data.our_node,
        request_id: String::default(),
        reciver_actor: format!("/user/node/{}/distributor", subject_string),
        schema: "governance".to_string(),
    };

    let helper: Option<Intermediary> = ctx.system().get_helper("network").await;

    let Some(mut helper) = helper else {
        let e = ActorError::NotHelper("network".to_owned());
        return Err(e);
    };

    helper
        .send_command(network::CommandHelper::SendMessage {
            message: NetworkMessage {
                info,
                message: request,
            },
        })
        .await
}

pub async fn update_ledger_local<A>(
    ctx: &mut ActorContext<A>,
    data: UpdateData,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let subject_string = data.subject_id.to_string();
    let request = ActorMessage::DistributionLedgerReq {
        gov_version: Some(data.gov_version),
        actual_sn: Some(data.sn),
        subject_id: data.subject_id,
    };

    let info = ComunicateInfo {
        reciver: data.other_node,
        sender: data.our_node,
        request_id: String::default(),
        reciver_actor: format!("/user/node/{}/distributor", subject_string),
        schema: "governance".to_string(),
    };

    let helper: Option<Intermediary> = ctx.system().get_helper("network").await;

    let Some(mut helper) = helper else {
        let e = ActorError::NotHelper("network".to_owned());
        return Err(e);
    };

    helper
        .send_command(network::CommandHelper::SendMessage {
            message: NetworkMessage {
                info,
                message: request,
            },
        })
        .await
}

pub async fn get_last_event<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<Signed<KoreEvent>, ActorError>
where
    A: Actor + Handler<A>,
{
    let ledger_event_path =
        ActorPath::from(format!("/user/node/{}/ledger_event", subject_id));
    let ledger_event_actor: Option<ActorRef<LedgerEvent>> =
        ctx.system().get_actor(&ledger_event_path).await;

    let response = if let Some(ledger_event_actor) = ledger_event_actor {
        ledger_event_actor
            .ask(LedgerEventMessage::GetLastEvent)
            .await?
    } else {
        return Err(ActorError::NotFound(ledger_event_path));
    };

    match response {
        LedgerEventResponse::LastEvent(event) => Ok(event),
        _ => Err(ActorError::UnexpectedResponse(
            ledger_event_path,
            "LedgerEventResponse::LastEvent".to_owned(),
        )),
    }
}

pub async fn send_reboot_to_req<A>(
    ctx: &mut ActorContext<A>,
    request_id: &str,
    governance_id: DigestIdentifier,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let req_path = ActorPath::from(format!("/user/request/{}", request_id));
    let req_actor: Option<ActorRef<RequestManager>> =
        ctx.system().get_actor(&req_path).await;

    if let Some(req_actor) = req_actor {
        req_actor
            .tell(RequestManagerMessage::Reboot { governance_id })
            .await?
    } else {
        return Err(ActorError::NotFound(req_path));
    };

    Ok(())
}
