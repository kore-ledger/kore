use rush::{
    Actor, ActorContext, ActorError, ActorPath, ActorRef, Handler, Store,
    StoreCommand, StoreResponse,
};

use identity::identifier::{DigestIdentifier, KeyIdentifier};
use network::ComunicateInfo;

use crate::{
    ActorMessage, NetworkMessage,
    auth::{Auth, AuthMessage, WitnessesAuth},
    db::Storable,
    intermediary::Intermediary,
    model::{
        event::{Event as KoreEvent, ProtocolsSignatures},
        signature::Signed,
    },
    node::{
        nodekey::{NodeKey, NodeKeyMessage, NodeKeyResponse},
        transfer::{
            TransferRegister, TransferRegisterMessage, TransferRegisterResponse,
        },
    },
    request::manager::{RequestManager, RequestManagerMessage},
    subject::laststate::{LastState, LastStateMessage, LastStateResponse},
    validation::proof::ValidationProof,
};

pub mod contract;
pub mod node;
pub mod relation_ship;

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

pub async fn get_last_storage<A>(
    ctx: &mut ActorContext<A>,
) -> Result<Option<A::Event>, ActorError>
where
    A: Actor + Handler<A> + Storable,
{
    let path = ActorPath::from(&format!("{}/store", ctx.path()));
    let store: Option<ActorRef<Store<A>>> = ctx.get_child("store").await;
    let response = if let Some(store) = store {
        store.ask(StoreCommand::LastEvent).await?
    } else {
        return Err(ActorError::NotFound(path));
    };

    match response {
        StoreResponse::LastEvent(event) => Ok(event),
        StoreResponse::Error(e) => {
            Err(ActorError::FunctionalFail(e.to_string()))
        }
        _ => Err(ActorError::UnexpectedResponse(
            path,
            "StoreResponse::LastEvent".to_string(),
        )),
    }
}

pub async fn get_storage_events<A>(
    ctx: &mut ActorContext<A>,
    last_event: u64,
    quantity: u64,
) -> Result<Vec<A::Event>, ActorError>
where
    A: Actor + Handler<A> + Storable,
{
    let store: Option<ActorRef<Store<A>>> = ctx.get_child("store").await;
    let response = if let Some(store) = store {
        store
            .ask(StoreCommand::GetEvents {
                from: last_event,
                to: last_event + quantity,
            })
            .await?
    } else {
        return Err(ActorError::NotFound(ActorPath::from(format!(
            "{}/store",
            ctx.path()
        ))));
    };

    match response {
        StoreResponse::Events(events) => Ok(events),
        _ => Err(ActorError::UnexpectedResponse(
            ActorPath::from(format!("{}/store", ctx.path())),
            "StoreResponse::Events".to_owned(),
        )),
    }
}

pub async fn get_node_key<A>(
    ctx: &mut ActorContext<A>,
) -> Result<KeyIdentifier, ActorError>
where
    A: Actor + Handler<A>,
{
    // Node path.
    let node_key_path = ActorPath::from("/user/node/key");
    // Node actor.
    let node_key_actor: Option<ActorRef<NodeKey>> =
        ctx.system().get_actor(&node_key_path).await;

    // We obtain the actor node
    let response = if let Some(node_key_actor) = node_key_actor {
        node_key_actor.ask(NodeKeyMessage::GetKeyIdentifier).await?
    } else {
        return Err(ActorError::NotFound(node_key_path));
    };

    // We handle the possible responses of node
    match response {
        NodeKeyResponse::KeyIdentifier(key) => Ok(key),
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
    let transfer_register_actor: Option<rush::ActorRef<TransferRegister>> =
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

pub async fn update_last_state<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
    event: Signed<KoreEvent>,
    proof: ValidationProof,
    vali_res: Vec<ProtocolsSignatures>,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A>,
{
    let last_state_path =
        ActorPath::from(format!("/user/node/{}/last_state", subject_id));
    let Some(last_state_actor): Option<ActorRef<LastState>> =
        ctx.system().get_actor(&last_state_path).await
    else {
        return Err(ActorError::NotFound(last_state_path));
    };
    let response = last_state_actor
        .ask(LastStateMessage::UpdateLastState {
            proof: Box::new(proof),
            event: Box::new(event),
            vali_res,
        })
        .await?;

    match response {
        LastStateResponse::Ok => Ok(()),
        _ => Err(ActorError::UnexpectedResponse(
            last_state_path,
            "LastStateResponse::Ok".to_owned(),
        )),
    }
}

pub async fn get_last_state<A>(
    ctx: &mut ActorContext<A>,
    subject_id: &str,
) -> Result<
    (ValidationProof, Signed<KoreEvent>, Vec<ProtocolsSignatures>),
    ActorError,
>
where
    A: Actor + Handler<A>,
{
    let last_state_path =
        ActorPath::from(format!("/user/node/{}/last_state", subject_id));
    let Some(last_state_actor): Option<ActorRef<LastState>> =
        ctx.system().get_actor(&last_state_path).await
    else {
        return Err(ActorError::NotFound(last_state_path));
    };
    let response = last_state_actor.ask(LastStateMessage::GetLastState).await?;

    match response {
        LastStateResponse::LastState {
            proof,
            event,
            vali_res,
        } => Ok((*proof, *event, vali_res)),
        _ => Err(ActorError::UnexpectedResponse(
            last_state_path,
            "LastStateResponse::LastState".to_owned(),
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

pub async fn purge_storage<A>(
    ctx: &mut ActorContext<A>,
) -> Result<(), ActorError>
where
    A: Actor + Handler<A> + Storable,
{
    let store: Option<ActorRef<Store<A>>> = ctx.get_child("store").await;
    let response = if let Some(store) = store {
        store.ask(StoreCommand::Purge).await?
    } else {
        return Err(ActorError::NotFound(ActorPath::from(format!(
            "{}/store",
            ctx.path()
        ))));
    };

    if let StoreResponse::Error(e) = response {
        return Err(ActorError::Store(format!("Can not purge request: {}", e)));
    };

    Ok(())
}

pub async fn emit_fail<A>(
    ctx: &mut ActorContext<A>,
    error: ActorError,
) -> ActorError
where
    A: Actor + Handler<A>,
{
    if let Err(e) = ctx.emit_fail(error.clone()).await {
        ctx.system().stop_system();
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
