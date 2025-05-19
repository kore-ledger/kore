// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use rand::rng;
use rand::seq::IteratorRandom;
use std::collections::{HashMap, HashSet};

use actor::{
    Actor, ActorContext, ActorPath, ActorRef, Error as ActorError, Handler,
    SystemEvent,
};

use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;
use wasmtime::{Caller, Engine, Linker};

use crate::{
    ActorMessage, Error, Event as KoreEvent, EventRequestType, Governance,
    NetworkMessage, Node, NodeMessage, NodeResponse, Signature, Signed,
    Subject, SubjectMessage, SubjectResponse,
    auth::{Auth, AuthMessage, WitnessesAuth},
    governance::{
        Quorum,
        model::{CreatorQuantity, ProtocolTypes},
    },
    intermediary::Intermediary,
    model::SignTypesNode,
    node::{
        SubjectData,
        relationship::{
            OwnerSchema, RelationShip, RelationShipMessage,
            RelationShipResponse,
        },
        transfer::{
            TransferRegister, TransferRegisterMessage, TransferRegisterResponse,
        },
    },
    request::manager::{RequestManager, RequestManagerMessage},
    subject::{
        Metadata,
        event::{LedgerEvent, LedgerEventMessage, LedgerEventResponse},
        validata::{ValiData, ValiDataMessage, ValiDataResponse},
    },
    validation::proof::ValidationProof,
};
use tracing::error;

use super::{Namespace, event::ProtocolsSignatures};

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

pub fn take_random_signers(
    signers: HashSet<KeyIdentifier>,
    quantity: usize,
) -> (HashSet<KeyIdentifier>, HashSet<KeyIdentifier>) {
    if quantity == signers.len() {
        return (signers, HashSet::new());
    }

    let mut rng = rng();

    let random_signers: HashSet<KeyIdentifier> = signers
        .iter()
        .choose_multiple(&mut rng, quantity)
        .into_iter()
        .cloned()
        .collect();

    let signers = signers
        .difference(&random_signers)
        .cloned()
        .collect::<HashSet<KeyIdentifier>>();

    (random_signers, signers)
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

pub async fn subject_owner<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<(bool, bool), ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<actor::ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    let response = if let Some(node_actor) = node_actor {
        node_actor
            .ask(NodeMessage::OwnerPendingSubject(subject_id.to_owned()))
            .await?
    } else {
        return Err(ActorError::NotFound(node_path));
    };

    match response {
        NodeResponse::IOwnerPending(res) => Ok(res),
        _ => Err(ActorError::UnexpectedResponse(
            node_path,
            "NodeResponse::OwnerPending".to_owned(),
        )),
    }
}

pub async fn subject_old<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<bool, ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<actor::ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    let response = if let Some(node_actor) = node_actor {
        node_actor
            .ask(NodeMessage::OldSubject(subject_id.to_owned()))
            .await?
    } else {
        return Err(ActorError::NotFound(node_path));
    };

    match response {
        NodeResponse::IOld(res) => Ok(res),
        _ => Err(ActorError::UnexpectedResponse(
            node_path,
            "NodeResponse::OwnerPending".to_owned(),
        )),
    }
}

pub async fn subject_old_owner<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
    old: KeyIdentifier,
) -> Result<bool, ActorError>
where
    A: Actor + Handler<A>,
{
    let tranfer_register_path = ActorPath::from("/user/node/transfer_register");
    let transfer_register_actor: Option<actor::ActorRef<TransferRegister>> =
        ctx.system().get_actor(&tranfer_register_path).await;

    let response =
        if let Some(transfer_register_actor) = transfer_register_actor {
            transfer_register_actor
                .ask(TransferRegisterMessage::IsOldOwner {
                    subject_id: subject_id.to_owned(),
                    old,
                })
                .await?
        } else {
            return Err(ActorError::NotFound(tranfer_register_path));
        };

    match response {
        TransferRegisterResponse::IsOwner(res) => Ok(res),
        _ => Err(ActorError::UnexpectedResponse(
            tranfer_register_path,
            "TransferRegisterResponse::IsOwner".to_owned(),
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

pub async fn get_node_subject_data<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<Option<(SubjectData, Option<String>)>, ActorError>
where
    A: Actor + Handler<A>,
{
    let node_path = ActorPath::from("/user/node");
    let node_actor: Option<ActorRef<Node>> =
        ctx.system().get_actor(&node_path).await;

    // We obtain the validator
    let node_response = if let Some(node_actor) = node_actor {
        node_actor
            .ask(NodeMessage::GetSubjectData(subject_id.to_owned()))
            .await?
    } else {
        return Err(ActorError::NotFound(node_path));
    };

    match node_response {
        NodeResponse::None => Ok(None),
        NodeResponse::SubjectData { data, new_owner } => {
            Ok(Some((data, new_owner)))
        }
        _ => Err(ActorError::UnexpectedResponse(
            node_path,
            "NodeResponse::SubjectData || NodeResponse::None".to_owned(),
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

pub async fn get_quantity<A>(
    ctx: &mut ActorContext<A>,
    gov: String,
    schema_id: String,
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
                schema_id,
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
    schema_id: String,
    owner: String,
    subject: String,
    namespace: String,
    max_quantity: CreatorQuantity,
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
                    schema_id,
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
    schema_id: String,
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
                    schema_id,
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
    is_gov: bool,
) -> Result<bool, Error> {
    match request {
        EventRequestType::Create
        | EventRequestType::EOL
        | EventRequestType::Reject => {
            if approve.is_some() || eval.is_some() || approval_require {
                return Err(Error::Protocols("In create, reject and eol request, approve and eval must be None and approval require must be false".to_owned()));
            }
            Ok(val)
        }
        EventRequestType::Transfer => {
            let Some(eval) = eval else {
                return Err(Error::Protocols(
                    "In Transfer even eval must be Some".to_owned(),
                ));
            };

            if approve.is_some() || approval_require {
                return Err(Error::Protocols("In transfer request, approve must be None and approval require must be false".to_owned()));
            }

            Ok(val && eval)
        }
        EventRequestType::Fact => {
            let Some(eval) = eval else {
                return Err(Error::Protocols(
                    "In fact request eval must be Some".to_owned(),
                ));
            };

            if !is_gov {
                if approve.is_some() || approval_require {
                    return Err(Error::Protocols("In fact request (not governace subject), approve must be None and approval require must be false".to_owned()));
                }

                Ok(val && eval)
            } else if eval {
                if !approval_require {
                    return Err(Error::Protocols("In fact request (governace subject), if eval is success approval require must be true".to_owned()));
                }
                let Some(approve) = approve else {
                    return Err(Error::Protocols("In fact request if approval was required, approve must be Some".to_owned()));
                };
                Ok(eval && approve && val)
            } else {
                if approval_require {
                    return Err(Error::Protocols("In fact request (governace subject), if eval is not success approval require must be false".to_owned()));
                }

                if approve.is_some() {
                    return Err(Error::Protocols("In fact request if approval was not required, approve must be None".to_owned()));
                }

                Ok(eval && val)
            }
        }
        EventRequestType::Confirm => {
            if !is_gov {
                if approve.is_some() || eval.is_some() || approval_require {
                    return Err(Error::Protocols("In confirm request (not governance subject), approve and eval must be None and approval require must be false".to_owned()));
                }
                Ok(val)
            } else {
                let Some(eval) = eval else {
                    return Err(Error::Protocols(
                        "In confirm request (governace subject) eval must be Some".to_owned(),
                    ));
                };

                if approve.is_some() || approval_require {
                    return Err(Error::Protocols("In confirm request (governace subject), approve must be None and approval require must be false".to_owned()));
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
    role: ProtocolTypes,
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

pub async fn try_to_update<A>(
    ctx: &mut ActorContext<A>,
    subject_id: DigestIdentifier,
    more_info: WitnessesAuth,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let auth_path = ActorPath::from("/user/node/auth");
    let auth_actor: Option<ActorRef<Auth>> =
        ctx.system().get_actor(&auth_path).await;

    if let Some(auth_actor) = auth_actor {
        auth_actor
            .tell(AuthMessage::Update {
                subject_id,
                more_info,
            })
            .await?;
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
                caller.data().get_pointer_len(pointer as usize)
                    as u32
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
                caller.data_mut().alloc(len as usize) as u32
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
                caller
                    .data_mut()
                    .write_byte(ptr as usize, offset as usize, data as u8)
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
                caller.data().read_byte(index as usize) as u32
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

pub async fn update_vali_data<A>(
    ctx: &mut ActorContext<A>,
    last_proof: ValidationProof,
    prev_event_validation_response: Vec<ProtocolsSignatures>,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let vali_data_path = ActorPath::from(format!(
        "/user/node/{}/vali_data",
        last_proof.subject_id
    ));
    let vali_data_actor: Option<ActorRef<ValiData>> =
        ctx.system().get_actor(&vali_data_path).await;

    let response = if let Some(vali_data_actor) = vali_data_actor {
        vali_data_actor
            .ask(ValiDataMessage::UpdateValiData {
                last_proof: Box::new(last_proof),
                prev_event_validation_response,
            })
            .await?
    } else {
        return Err(ActorError::NotFound(vali_data_path));
    };

    match response {
        ValiDataResponse::Ok => Ok(()),
        _ => Err(ActorError::UnexpectedResponse(
            vali_data_path,
            "ValiDataResponse::Ok".to_owned(),
        )),
    }
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
        version: 0,
        reciver_actor: format!("/user/node/distributor_{}", subject_string),
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

pub async fn get_vali_data<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<(Option<ValidationProof>, Vec<ProtocolsSignatures>), ActorError>
where
    A: Actor + Handler<A>,
{
    let vali_data_path =
        ActorPath::from(format!("/user/node/{}/vali_data", subject_id));
    let vali_data_actor: Option<ActorRef<ValiData>> =
        ctx.system().get_actor(&vali_data_path).await;

    let response = if let Some(vali_data_actor) = vali_data_actor {
        vali_data_actor
            .ask(ValiDataMessage::GetLastValiData)
            .await?
    } else {
        return Err(ActorError::NotFound(vali_data_path));
    };

    match response {
        ValiDataResponse::LastValiData {
            last_proof,
            prev_event_validation_response,
        } => Ok((*last_proof, prev_event_validation_response)),
        _ => Err(ActorError::UnexpectedResponse(
            vali_data_path,
            "ValiDataResponse::LastValiData".to_owned(),
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
