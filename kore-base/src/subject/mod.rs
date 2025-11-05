//! # Subject module.
//!

use crate::{
    CreateRequest, Error, EventRequestType, Governance, Node,
    auth::WitnessesAuth,
    db::Storable,
    governance::{
        GovernanceMessage, GovernanceResponse,
        data::GovernanceData,
        model::{ProtocolTypes, Quorum},
    },
    helpers::{db::ExternalDB, sink::KoreSink},
    model::{
        HashId, Namespace, ValueWrapper,
        common::{
            emit_fail, get_last_state, get_storage_events, try_to_update,
        },
        event::{Event as KoreEvent, Ledger, LedgerValue, ProtocolsSignatures},
        request::{EventRequest},
        signature::Signed,
    },
    node::{
        NodeMessage, TransferSubject,
        register::{Register, RegisterMessage},
        transfer::{TransferRegister, TransferRegisterMessage},
    },
    update::TransferResponse,
    validation::proof::ValidationProof,
};

use identity::identifier::{
    DigestIdentifier, KeyIdentifier, derive::digest::DigestDerivator,
};
use rand::{rng, seq::IteratorRandom};
use rush::{
    Actor, ActorContext, ActorError, ActorPath, ActorRef, ChildAction, Event,
    Handler, Message, Response, Sink,
};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use json_patch::{Patch, patch};
use rush::{FullPersistence, PersistentActor};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, to_value};
use sinkdata::{SinkData, SinkDataMessage};
use tracing::{error, warn};

use std::collections::{BTreeMap, HashMap, HashSet};

pub mod validata;
pub mod event;
pub mod sinkdata;

const TARGET_SUBJECT: &str = "Kore-Subject";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateSubjectData {
    pub create_req: CreateRequest,
    pub subject_id: DigestIdentifier,
    pub creator: KeyIdentifier,
}

/// Subject metadata.
#[derive(
    Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
pub struct Metadata {
    pub name: Option<String>,
    pub description: Option<String>,
    /// The identifier of the subject of the event.
    pub subject_id: DigestIdentifier,
    pub governance_id: DigestIdentifier,
    pub genesis_gov_version: u64,
    pub last_event_hash: DigestIdentifier,
    /// The identifier of the schema_id used to validate the event.
    pub schema_id: String,
    /// The namespace of the subject.
    pub namespace: Namespace,
    /// The current sequence number of the subject.
    pub sn: u64,
    /// The identifier of the public key of the creator owner.
    pub creator: KeyIdentifier,
    /// The identifier of the public key of the subject owner.
    pub owner: KeyIdentifier,

    pub new_owner: Option<KeyIdentifier>,
    /// Indicates whether the subject is active or not.
    pub active: bool,
    /// The current status of the subject.
    pub properties: ValueWrapper,
}

impl From<Governance> for Metadata {
    fn from(value: Governance) -> Self {
        Metadata {
            name: value.subject_metadata.name,
            description: value.subject_metadata.description,
            subject_id: value.subject_metadata.subject_id,
            governance_id: DigestIdentifier::default(),
            genesis_gov_version: 0,
            last_event_hash: value.subject_metadata.last_event_hash,
            schema_id: value.subject_metadata.schema_id,
            namespace: Namespace::default(),
            sn: value.subject_metadata.sn,
            creator: value.subject_metadata.creator,
            owner: value.subject_metadata.owner,
            new_owner: value.subject_metadata.new_owner,
            active: value.subject_metadata.active,
            properties: ValueWrapper(json!(value.properties)),
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct SubjectMetadata {
    /// The name of the subject.
    pub name: Option<String>,
    /// The description of the subject.
    pub description: Option<String>,
    /// The identifier of the subject.
    pub subject_id: DigestIdentifier,

    pub schema_id: String,
    /// The identifier of the public key of the subject owner.
    pub owner: KeyIdentifier,
    /// The identifier of the public key of the new subject owner.
    pub new_owner: Option<KeyIdentifier>,

    pub last_event_hash: DigestIdentifier,
    /// The identifier of the public key of the subject creator.
    pub creator: KeyIdentifier,
    /// Indicates whether the subject is active or not.
    pub active: bool,
    /// The current sequence number of the subject.
    pub sn: u64,
}

impl SubjectMetadata {
    pub fn new(data: &CreateSubjectData) -> Self {
        Self {
            name: data.create_req.name.clone(),
            description: data.create_req.description.clone(),
            subject_id: data.subject_id.clone(),
            owner: data.creator.clone(),
            schema_id: data.create_req.schema_id.clone(),
            new_owner: None,
            last_event_hash: DigestIdentifier::default(),
            creator: data.creator.clone(),
            active: true,
            sn: 0,
        }
    }
    pub fn from_create_request(
        subject_id: DigestIdentifier,
        request: CreateRequest,
        owner: KeyIdentifier,
    ) -> Self {
        Self {
            name: request.name.clone(),
            description: request.description.clone(),
            subject_id: subject_id,
            owner: owner.clone(),
            schema_id: request.schema_id,
            new_owner: None,
            last_event_hash: DigestIdentifier::default(),
            creator: owner.clone(),
            active: true,
            sn: 0,
        }
    }
}

#[async_trait]
pub trait Subject {
    fn verify_protocols_state(
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

    async fn get_signers_quorum_gov_version<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
        schema_id: &str,
        namespace: Namespace,
        role: ProtocolTypes,
    ) -> Result<(HashSet<KeyIdentifier>, Quorum, u64), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let gov = Self::get_gov(ctx, subject_id, schema_id.is_empty()).await?;

        let (signers, quorum) =
            gov.get_quorum_and_signers(role, schema_id, namespace)?;
        Ok((signers, quorum, gov.version))
    }

    fn take_random_signers(
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

    async fn get_gov<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
        is_gov: bool,
    ) -> Result<GovernanceData, ActorError>
    where
        A: Actor + Handler<A>,
    {
        // Subject path.
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));

        // Governance actor.
        if is_gov {
            let Some(governance_actor) =
                ctx.system().get_actor::<Governance>(&subject_path).await
            else {
                return Err(ActorError::NotFound(subject_path));
            };

            // We obtain the actor governance
            let response = governance_actor
                .ask(GovernanceMessage::GetGovernance)
                .await?;

            match response {
                GovernanceResponse::Governance(gov) => Ok(*gov),
                _ => Err(ActorError::UnexpectedResponse(
                    subject_path,
                    "GovernanceResponse::Governance".to_owned(),
                )),
            }
        } else {
            /*
                let Some(tracker_actor) =
                    ctx.system().get_actor::<Tracker>(&subject_path).await else {
                        return Err(ActorError::NotFound(subject_path));
                    };

                let response = tracker_actor.ask(TracerMessage::GetGovernance).await?;

                match response {
                    TrackerResponse::Governance(gov) => Ok(*gov),
                    _ => Err(ActorError::UnexpectedResponse(
                        subject_path,
                        "SubjectResponse::Governance".to_owned(),
                    )),
                }
            */
            todo!()
        }
    }

    async fn get_metadata<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
        is_gov: bool,
    ) -> Result<Metadata, ActorError>
    where
        A: Actor + Handler<A>,
    {
        // Subject path.
        let subject_path =
            ActorPath::from(format!("/user/node/{}", subject_id));

        // Governance actor.
        if is_gov {
            let Some(governance_actor) =
                ctx.system().get_actor::<Governance>(&subject_path).await
            else {
                return Err(ActorError::NotFound(subject_path));
            };

            // We obtain the actor governance
            let response =
                governance_actor.ask(GovernanceMessage::GetMetadata).await?;

            match response {
                GovernanceResponse::Metadata(metadata) => Ok(*metadata),
                _ => Err(ActorError::UnexpectedResponse(
                    subject_path,
                    "GovernanceResponse::Metadata".to_owned(),
                )),
            }
        } else {
            /*
                let Some(tracker_actor) =
                    ctx.system().get_actor::<Tracker>(&subject_path).await else {
                        return Err(ActorError::NotFound(subject_path));
                    };

                let response = tracker_actor.ask(TracerMessage::GetGovernance).await?;

                match response {
                    TrackerResponse::Governance(gov) => Ok(*gov),
                    _ => Err(ActorError::UnexpectedResponse(
                        subject_path,
                        "SubjectResponse::Governance".to_owned(),
                    )),
                }
            */
            todo!()
        }
    }

    async fn verify_create_event(
        owner: KeyIdentifier,
        event: &Signed<Ledger>,
        is_gov: bool,
    ) -> Result<(), Error> {
        if let EventRequest::Create(event_req) =
            event.content.event_request.content.clone()
        {
            if let Some(name) = event_req.name
                && (name.is_empty() || name.len() > 100)
            {
                return Err(Error::Subject("The subject name must be less than 100 characters or not be empty.".to_owned()));
            }

            if let Some(description) = event_req.description
                && (description.is_empty() || description.len() > 200)
            {
                return Err(Error::Subject("The subject description must be less than 200 characters or not be empty.".to_owned()));
            }

            if is_gov && event_req.schema_id != "governance" {
                return Err(Error::Subject("In create event, is a governance but create event schema_id is not governance".to_owned()));
            }

            if !is_gov && event_req.schema_id == "governance" {
                return Err(Error::Subject("In create event, is not a governance but create event schema_id is governance".to_owned()));
            }

            if is_gov
                && (!event_req.governance_id.is_empty()
                    || !event_req.namespace.is_empty()
                    || event.content.gov_version != 0)
            {
                return Err(Error::Subject("In create event, for governance, governance_id must be empty, namespace must be empty and gov version must be 0".to_owned()));
            }

            if !is_gov
                && (event_req.governance_id.is_empty()
                    || event.content.gov_version == 0)
            {
                return Err(Error::Subject("In create event, for schema, governance_id can not be empty and gov version can not be 0".to_owned()));
            }
        } else {
            return Err(Error::Subject(
                "First event is not a create event".to_owned(),
            ));
        };

        if event.signature.signer != owner
            || event.content.event_request.signature.signer != owner
        {
            return Err(Error::Subject(
                "In create event, owner must sign request and event."
                    .to_owned(),
            ));
        }

        if let Err(e) = event.verify() {
            return Err(Error::Subject(format!(
                "In create event, event signature: {}",
                e
            )));
        }

        if let Err(e) = event.content.event_request.verify() {
            return Err(Error::Subject(format!(
                "In create event, request signature: {}",
                e
            )));
        }

        if event.content.sn != 0 {
            return Err(Error::Subject(
                "In create event, sn must be 0.".to_owned(),
            ));
        }

        if !event.content.hash_prev_event.is_empty() {
            return Err(Error::Subject(
                "In create event, previous hash event must be empty."
                    .to_owned(),
            ));
        }

        if Self::verify_protocols_state(
            EventRequestType::Create,
            event.content.eval_success,
            event.content.appr_success,
            event.content.appr_required,
            event.content.vali_success,
            is_gov,
        )? {
            Ok(())
        } else {
            Err(Error::Subject(
                "Create event fail in validation protocol".to_owned(),
            ))
        }
    }

    fn verify_event(
        subject_metadata: &SubjectMetadata,
        properties: &ValueWrapper,
        last_ledger: &Signed<Ledger>,
        new_ledger: &Signed<Ledger>,
    ) -> Result<bool, Error> {
        // Si no sigue activo
        if !subject_metadata.active {
            return Err(Error::Subject("Subject is not active".to_owned()));
        }

        if !new_ledger
            .content
            .event_request
            .content
            .check_ledger_signature(
                &new_ledger.signature.signer,
                &subject_metadata.owner,
                &subject_metadata.new_owner,
            )?
        {
            return Err(Error::Subject("Invalid ledger signer".to_owned()));
        }

        if !new_ledger
            .content
            .event_request
            .content
            .check_ledger_signature(
                &new_ledger.content.event_request.signature.signer,
                &subject_metadata.owner,
                &subject_metadata.new_owner,
            )?
        {
            return Err(Error::Subject("Invalid event signer".to_owned()));
        }

        if let Err(e) = new_ledger.verify() {
            return Err(Error::Subject(format!(
                "In new event, event signature: {}",
                e
            )));
        }

        if let Err(e) = new_ledger.content.event_request.verify() {
            return Err(Error::Subject(format!(
                "In new event request, request signature: {}",
                e
            )));
        }

        // Mirar que sea el siguiente sn
        if last_ledger.content.sn + 1 != new_ledger.content.sn {
            return Err(Error::Sn);
        }

        //Comprobar que el hash del actual event sea el mismo que el pre_event_hash,
        let last_ledger_hash = last_ledger
            .hash_id(last_ledger.signature.content_hash.derivator)?;

        if last_ledger_hash != new_ledger.content.hash_prev_event {
            return Err(Error::Subject("Last event hash is not the same that previous event hash in new event".to_owned()));
        }

        let valid_last_event = Self::verify_protocols_state(
            EventRequestType::from(
                last_ledger.content.event_request.content.clone(),
            ),
            last_ledger.content.eval_success,
            last_ledger.content.appr_success,
            last_ledger.content.appr_required,
            last_ledger.content.vali_success,
            subject_metadata.schema_id.is_empty(),
        )?;

        if valid_last_event
            && let EventRequest::EOL(..) =
                last_ledger.content.event_request.content.clone()
        {
            return Err(Error::Subject(
                "The last event was EOL, no more events can be received"
                    .to_owned(),
            ));
        }

        let valid_new_event = Self::verify_protocols_state(
            EventRequestType::from(
                new_ledger.content.event_request.content.clone(),
            ),
            new_ledger.content.eval_success,
            new_ledger.content.appr_success,
            new_ledger.content.appr_required,
            new_ledger.content.vali_success,
            subject_metadata.schema_id.is_empty(),
        )?;

        // Si el nuevo evento a registrar fue correcto.
        if valid_new_event {
            match new_ledger.content.event_request.content.clone() {
                EventRequest::Create(_start_request) => {
                    return Err(Error::Subject("A creation event is being logged when the subject has already been created previously".to_owned()));
                }
                EventRequest::Fact(_fact_request) => {
                    if subject_metadata.new_owner.is_some() {
                        return Err(Error::Subject("After a transfer event there must be a confirmation or a reject event.".to_owned()));
                    }

                    Self::check_patch(
                        properties.0.clone(),
                        &new_ledger.content.value,
                        &new_ledger.content.state_hash,
                        &new_ledger.signature.content_hash.derivator,
                    )?;
                }
                EventRequest::Transfer(..) => {
                    if subject_metadata.new_owner.is_some() {
                        return Err(Error::Subject("After a transfer event there must be a confirmation or a reject event.".to_owned()));
                    }

                    let hash_without_patch = properties
                        .hash_id(new_ledger.signature.content_hash.derivator)?;

                    if hash_without_patch != new_ledger.content.state_hash {
                        return Err(Error::Subject("In Transfer event, the hash obtained without applying any patch is different from the state hash of the event".to_owned()));
                    }
                }
                EventRequest::Confirm(..) => {
                    if subject_metadata.new_owner.is_none() {
                        return Err(Error::Subject("Before a confirm event there must be a transfer event.".to_owned()));
                    }

                    if subject_metadata.schema_id.is_empty() {
                        Self::check_patch(
                            properties.0.clone(),
                            &new_ledger.content.value,
                            &new_ledger.content.state_hash,
                            &new_ledger.signature.content_hash.derivator,
                        )?;
                    } else {
                        let hash_without_patch = properties.hash_id(
                            new_ledger.signature.content_hash.derivator,
                        )?;

                        if hash_without_patch != new_ledger.content.state_hash {
                            return Err(Error::Subject("In Confirm event, the hash obtained without applying any patch is different from the state hash of the event".to_owned()));
                        }
                    }
                }
                EventRequest::Reject(..) => {
                    if subject_metadata.new_owner.is_none() {
                        return Err(Error::Subject("Before a reject event there must be a transfer event.".to_owned()));
                    }

                    let hash_without_patch = properties
                        .hash_id(new_ledger.signature.content_hash.derivator)?;

                    if hash_without_patch != new_ledger.content.state_hash {
                        return Err(Error::Subject("In Reject event, the hash obtained without applying any patch is different from the state hash of the event".to_owned()));
                    }
                }
                EventRequest::EOL(..) => {
                    if subject_metadata.new_owner.is_some() {
                        return Err(Error::Subject("After a transfer event there must be a confirmation or a reject event.".to_owned()));
                    }

                    let hash_without_patch = properties
                        .hash_id(new_ledger.signature.content_hash.derivator)?;

                    if hash_without_patch != new_ledger.content.state_hash {
                        return Err(Error::Subject("In EOL event, the hash obtained without applying any patch is different from the state hash of the event".to_owned()));
                    }
                }
            };
        } else {
            let hash_without_patch = properties
                .hash_id(new_ledger.signature.content_hash.derivator)?;

            if hash_without_patch != new_ledger.content.state_hash {
                return Err(Error::Subject("The hash obtained without applying any patch is different from the state hash of the event".to_owned()));
            }
        }

        Ok(valid_new_event)
    }

    async fn register<A>(
        ctx: &mut ActorContext<A>,
        message: RegisterMessage,
    ) -> Result<(), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let register_path = ActorPath::from("/user/node/register");
        let register: Option<ActorRef<Register>> =
            ctx.system().get_actor(&register_path).await;
        if let Some(register) = register {
            /*
                let message = if self.governance_id.is_empty() {
                    RegisterMessage::RegisterGov {
                        gov_id: self.subject_id.to_string(),
                        data: RegisterDataGov {
                            active,
                            name: self.name.clone(),
                            description: self.description.clone(),
                        },
                    }
                } else {
                    RegisterMessage::RegisterSubj {
                        gov_id: self.governance_id.to_string(),
                        data: RegisterDataSubj {
                            subject_id: self.subject_id.to_string(),
                            schema_id: self.schema_id.clone(),
                            active,
                            name: self.name.clone(),
                            description: self.description.clone(),
                        },
                    }
                };
            */

            register.tell(message).await?;
        } else {
            return Err(ActorError::NotFound(register_path));
        }

        Ok(())
    }

    async fn publish_sink<A>(
        ctx: &mut ActorContext<A>,
        message: SinkDataMessage,
    ) -> Result<(), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let sink_data: Option<ActorRef<SinkData>> =
            ctx.get_child("sink_data").await;
        if let Some(sink_data) = sink_data {
            sink_data.tell(message).await
        } else {
            Err(ActorError::NotFound(ActorPath::from(format!(
                "{}/sink_data",
                ctx.path()
            ))))
        }
    }

    async fn delet_subject_from_node<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
    ) -> Result<(), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        // We obtain the validator
        let Some(node_actor) = node_actor else {
            return Err(ActorError::NotFound(node_path));
        };
        node_actor
            .tell(NodeMessage::DeleteSubject(subject_id.to_owned()))
            .await
    }

    async fn transfer_register<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
        new: KeyIdentifier,
        old: KeyIdentifier,
    ) -> Result<(), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let tranfer_register_path =
            ActorPath::from("/user/node/transfer_register");
        let transfer_register_actor: Option<rush::ActorRef<TransferRegister>> =
            ctx.system().get_actor(&tranfer_register_path).await;

        let Some(transfer_register_actor) = transfer_register_actor else {
            return Err(ActorError::NotFound(tranfer_register_path));
        };

        transfer_register_actor
            .tell(TransferRegisterMessage::RegisterNewOldOwner {
                old,
                new,
                subject_id: subject_id.to_owned(),
            })
            .await?;

        Ok(())
    }

    async fn new_transfer_subject<A>(
        ctx: &mut ActorContext<A>,
        name: Option<String>,
        subject_id: &str,
        new_owner: &str,
        actual_owner: &str,
    ) -> Result<(), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        if let Some(node_actor) = node_actor {
            node_actor
                .tell(NodeMessage::TransferSubject(TransferSubject {
                    name: name.unwrap_or_default(),
                    subject_id: subject_id.to_owned(),
                    new_owner: new_owner.to_owned(),
                    actual_owner: actual_owner.to_owned(),
                }))
                .await?;
        } else {
            return Err(ActorError::NotFound(node_path));
        }
        Ok(())
    }

    async fn change_node_subject<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
        new_owner: &str,
        old_owner: &str,
    ) -> Result<(), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        if let Some(node_actor) = node_actor {
            node_actor
                .ask(NodeMessage::ChangeSubjectOwner {
                    new_owner: new_owner.to_owned(),
                    old_owner: old_owner.to_owned(),
                    subject_id: subject_id.to_owned(),
                })
                .await?;
        } else {
            return Err(ActorError::NotFound(node_path));
        }

        Ok(())
    }

    async fn reject_transfer_subject<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
    ) -> Result<(), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        if let Some(node_actor) = node_actor {
            node_actor
                .tell(NodeMessage::RejectTransfer(subject_id.to_owned()))
                .await?;
        } else {
            return Err(ActorError::NotFound(node_path));
        }
        Ok(())
    }

    async fn update_subject_node<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
        sn: u64,
    ) -> Result<(), ActorError>
    where
        A: Actor + Handler<A>,
    {
        let node_path = ActorPath::from("/user/node");
        let node_actor: Option<ActorRef<Node>> =
            ctx.system().get_actor(&node_path).await;

        if let Some(node_actor) = node_actor {
            node_actor
                .tell(NodeMessage::UpdateSubject {
                    subject_id: subject_id.to_owned(),
                    sn,
                })
                .await
        } else {
            Err(ActorError::NotFound(node_path))
        }
    }

    async fn get_ledger_data<A>(
        ctx: &mut ActorContext<A>,
        subject_id: &str,
        last_sn: u64,
        quantity: u64,
    ) -> Result<
        (
            Vec<A::Event>,
            Option<Signed<KoreEvent>>,
            Option<ValidationProof>,
            Option<Vec<ProtocolsSignatures>>,
        ),
        ActorError,
    >
    where
        A: Actor + Handler<A> + Storable,
    {
        let ledger = get_storage_events(ctx, quantity, last_sn).await?;

        if ledger.len() < quantity as usize {
            let (proof, event, vali_res) =
                get_last_state(ctx, subject_id).await.map_err(|e| {
                    error!(
                        TARGET_SUBJECT,
                        "GetLedger, can not get last event: {}", e
                    );
                    e
                })?;

            Ok((ledger, Some(event), Some(proof), Some(vali_res)))
        } else {
            Ok((ledger, None, None, None))
        }
    }

    fn check_patch(
        properties: Value,
        value: &LedgerValue,
        state_hash: &DigestIdentifier,
        derivator: &DigestDerivator,
    ) -> Result<(), Error> {
        let LedgerValue::Patch(json_patch) = value.clone() else {
            return Err(Error::Subject(
            "The event was successful but does not have a json patch to apply"
                .to_owned(),
        ));
        };

        let patch_json = serde_json::from_value::<Patch>(json_patch.0)
            .map_err(|e| {
                Error::Subject(format!("Failed to extract event patch: {}", e))
            })?;

        let mut properties = properties;
        let Ok(()) = patch(&mut properties, &patch_json) else {
            return Err(Error::Subject(
                "Failed to apply event patch".to_owned(),
            ));
        };

        let hash_state_after_patch =
            ValueWrapper(properties).hash_id(*derivator)?;

        if hash_state_after_patch != *state_hash {
            return Err(Error::Subject("The new patch has been applied and we have obtained a different hash than the event after applying the patch".to_owned()));
        }
        Ok(())
    }

    fn apply_patch(&mut self, value: LedgerValue) -> Result<(), ActorError> {
        let json_patch = match value {
            LedgerValue::Patch(value_wrapper) => value_wrapper,
            LedgerValue::Error(e) => {
                let error = format!(
                    "Apply, event value can not be an error if protocols was successful: {:?}",
                    e
                );
                error!(TARGET_SUBJECT, error);
                return Err(ActorError::Functional(error));
            }
        };

        let patch_json = match serde_json::from_value::<Patch>(json_patch.0) {
            Ok(patch) => patch,
            Err(e) => {
                let error = format!("Apply, can not obtain json patch: {}", e);
                error!(TARGET_SUBJECT, error);
                return Err(ActorError::Functional(error));
            }
        };

        if let Err(e) = patch(&mut self.properties.0, &patch_json) {
            let error = format!("Apply, can not apply json patch: {}", e);
            error!(TARGET_SUBJECT, error);
            return Err(ActorError::Functional(error));
        };

        Ok(())
    }
}

/// Subject command.
#[derive(Debug, Clone)]
pub enum SubjectMessage {
    UpdateTransfer(TransferResponse),
    CreateCompilers(Vec<String>),
    /// Get the subject metadata.
    GetMetadata,
    GetLedger {
        last_sn: u64,
    },
    GetLastLedger,
    UpdateLedger {
        events: Vec<Signed<Ledger>>,
    },
    /// Get governance if subject is a governance
    GetGovernance,
    GetOwner,
    DeleteSubject,
}

impl Message for SubjectMessage {}

/// Subject response.
#[derive(Debug, Clone)]
pub enum SubjectResponse {
    /// The subject metadata.
    Metadata(Box<Metadata>),
    UpdateResult(u64, KeyIdentifier, Option<KeyIdentifier>),
    Ledger {
        ledger: Vec<Signed<Ledger>>,
        last_event: Box<Option<Signed<KoreEvent>>>,
        last_proof: Box<Option<ValidationProof>>,
        prev_event_validation_response: Option<Vec<ProtocolsSignatures>>,
    },
    Governance(Box<Governance>),
    Owner(KeyIdentifier),
    NewCompilers(Vec<String>),
    Ok,
}

impl Response for SubjectResponse {}

/// Actor implementation for `Subject`.
#[async_trait]
impl Actor for Subject {
    type Event = Signed<Ledger>;
    type Message = SubjectMessage;
    type Response = SubjectResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("subject", None, true, ctx).await?;

        let our_key = self.get_node_key(ctx).await?;

        let Some(ext_db): Option<ExternalDB> =
            ctx.system().get_helper("ext_db").await
        else {
            return Err(ActorError::NotHelper("ext_db".to_owned()));
        };

        let Some(kore_sink): Option<KoreSink> =
            ctx.system().get_helper("sink").await
        else {
            return Err(ActorError::NotHelper("sink".to_owned()));
        };

        let ledger_event = LedgerEvent::new(self.governance_id.is_empty());
        let ledger_event_actor =
            ctx.create_child("ledger_event", ledger_event).await?;

        let sink = Sink::new(
            ledger_event_actor.subscribe(),
            ext_db.get_ledger_event(),
        );
        ctx.system().run_sink(sink).await;

        let vali_data = ValiData::default();
        let vali_data_actor = ctx.create_child("vali_data", vali_data).await?;
        let sink =
            Sink::new(vali_data_actor.subscribe(), ext_db.get_vali_data());
        ctx.system().run_sink(sink).await;

        if self.active {
            if self.governance_id.is_empty() {
                self.build_childs_governance(
                    ctx,
                    our_key.clone(),
                    ext_db.clone(),
                )
                .await?;
            } else {
                self.build_childs_all_schemas(ctx, our_key.clone()).await?;
            }
        }

        let sink_actor = ctx
            .create_child(
                "sink_data",
                SinkData {
                    controller_id: our_key.to_string(),
                },
            )
            .await?;
        let sink = Sink::new(sink_actor.subscribe(), ext_db.get_sink_data());
        ctx.system().run_sink(sink).await;

        let sink = Sink::new(sink_actor.subscribe(), kore_sink.clone());
        ctx.system().run_sink(sink).await;

        Ok(())
    }

    async fn pre_stop(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.stop_store(ctx).await
    }
}

#[async_trait]
impl Handler<Subject> for Subject {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: SubjectMessage,
        ctx: &mut ActorContext<Subject>,
    ) -> Result<SubjectResponse, ActorError> {
        match msg {
            SubjectMessage::UpdateTransfer(res) => {
                match res {
                    TransferResponse::Confirm => {
                        let Some(new_owner) = self.new_owner.clone() else {
                            let e = "Can not obtain new_owner";
                            error!(TARGET_SUBJECT, "Confirm, {}", e);
                            return Err(ActorError::Functional(e.to_owned()));
                        };

                        Subject::change_node_subject(
                            ctx,
                            &self.subject_id.to_string(),
                            &new_owner.to_string(),
                            &self.owner.to_string(),
                        )
                        .await?;

                        delete_relation(
                            ctx,
                            self.governance_id.to_string(),
                            self.schema_id.clone(),
                            self.owner.to_string(),
                            self.subject_id.to_string(),
                            self.namespace.to_string(),
                        )
                        .await?;
                    }
                    TransferResponse::Reject => {
                        Subject::reject_transfer_subject(
                            ctx,
                            &self.subject_id.to_string(),
                        )
                        .await?;
                        try_to_update(
                            ctx,
                            self.subject_id.clone(),
                            WitnessesAuth::None,
                        )
                        .await?;
                    }
                }

                Ok(SubjectResponse::Ok)
            }
            SubjectMessage::DeleteSubject => {
                self.delete_subject(ctx).await?;
                Ok(SubjectResponse::Ok)
            }
            SubjectMessage::CreateCompilers(compilers) => {
                let new_compilers =
                    match Self::create_compilers(ctx, &compilers).await {
                        Ok(new_compilers) => new_compilers,
                        Err(e) => {
                            warn!(
                                TARGET_SUBJECT,
                                "CreateCompilers, can not create compilers: {}",
                                e
                            );
                            return Err(e);
                        }
                    };
                Ok(SubjectResponse::NewCompilers(new_compilers))
            }
            SubjectMessage::GetLedger { last_sn } => {
                self.get_ledger_data(ctx, last_sn).await
            }
            SubjectMessage::GetLastLedger => {
                self.get_ledger_data(ctx, self.sn).await
            }
            SubjectMessage::GetOwner => {
                Ok(SubjectResponse::Owner(self.owner.clone()))
            }
            SubjectMessage::GetMetadata => {
                Ok(SubjectResponse::Metadata(Box::new(self.get_metadata())))
            }
            SubjectMessage::UpdateLedger { events } => {
                if let Err(e) =
                    self.verify_new_ledger_events(ctx, events.as_slice()).await
                {
                    warn!(
                        TARGET_SUBJECT,
                        "UpdateLedger, can not verify new events: {}", e
                    );
                    return Err(e);
                };
                Ok(SubjectResponse::UpdateResult(
                    self.sn,
                    self.owner.clone(),
                    self.new_owner.clone(),
                ))
            }
            SubjectMessage::GetGovernance => {
                // If is a governance
                if self.governance_id.is_empty() {
                    match Governance::try_from(self.properties.clone()) {
                        Ok(gov) => {
                            return Ok(SubjectResponse::Governance(Box::new(
                                gov,
                            )));
                        }
                        Err(e) => {
                            error!(
                                TARGET_SUBJECT,
                                "GetGovernance, can not convert governance from properties: {}",
                                e
                            );
                            return Err(ActorError::FunctionalFail(
                                e.to_string(),
                            ));
                        }
                    }
                }
                // If is not a governance
                return Ok(SubjectResponse::Governance(Box::new(
                    self.get_governance_from_other_subject(ctx).await?,
                )));
            }
        }
    }

    async fn on_event(
        &mut self,
        event: Signed<Ledger>,
        ctx: &mut ActorContext<Subject>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            error!(
                TARGET_SUBJECT,
                "OnEvent, can not persist information: {}", e
            );
            emit_fail(ctx, e).await;
        };

        if let Err(e) = ctx.publish_event(event).await {
            error!(
                TARGET_SUBJECT,
                "PublishEvent, can not publish event: {}", e
            );
            emit_fail(ctx, e).await;
        }
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Subject>,
    ) -> ChildAction {
        error!(TARGET_SUBJECT, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}

#[async_trait]
impl PersistentActor for Subject {
    type Persistence = FullPersistence;

    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        let valid_event = match verify_protocols_state(
            EventRequestType::from(event.content.event_request.content.clone()),
            event.content.eval_success,
            event.content.appr_success,
            event.content.appr_required,
            event.content.vali_success,
            self.governance_id.is_empty(),
        ) {
            Ok(is_ok) => is_ok,
            Err(e) => {
                let error =
                    format!("Apply, can not verify protocols state: {}", e);
                error!(TARGET_SUBJECT, error);
                return Err(ActorError::Functional(error));
            }
        };

        if valid_event {
            match &event.content.event_request.content {
                EventRequest::Create(_start_request) => {
                    let last_event_hash = match event
                        .hash_id(event.signature.content_hash.derivator)
                    {
                        Ok(hash) => hash,
                        Err(e) => {
                            let error = format!(
                                "Apply, can not obtain last event hash id: {}",
                                e
                            );
                            error!(TARGET_SUBJECT, error);
                            return Err(ActorError::Functional(error));
                        }
                    };

                    if self.schema_id != "governance" {
                        let properties = match event.content.value.clone() {
                            LedgerValue::Patch(value_wrapper) => value_wrapper,
                            LedgerValue::Error(e) => {
                                let error = format!(
                                    "Apply, event value can not be an error if protocols was successful: {:?}",
                                    e
                                );
                                error!(TARGET_SUBJECT, error);
                                return Err(ActorError::Functional(error));
                            }
                        };

                        self.properties = properties;
                    }

                    self.last_event_hash = last_event_hash;
                    return Ok(());
                }
                EventRequest::Fact(_fact_request) => {
                    self.apply_patch(event.content.value.clone())?;
                }
                EventRequest::Transfer(transfer_request) => {
                    self.new_owner = Some(transfer_request.new_owner.clone());
                }
                EventRequest::Confirm(_confirm_request) => {
                    if self.governance_id.is_empty() {
                        self.apply_patch(event.content.value.clone())?;
                    }

                    let Some(new_owner) = self.new_owner.clone() else {
                        let error = "In confirm event was succefully but new owner is empty:";
                        error!(TARGET_SUBJECT, error);
                        return Err(ActorError::Functional(error.to_owned()));
                    };

                    self.owner = new_owner;
                    self.new_owner = None;
                }
                EventRequest::Reject(_reject_request) => {
                    self.new_owner = None;
                }
                EventRequest::EOL(_eolrequest) => self.active = false,
            }

            if self.governance_id.is_empty() {
                let mut gov = match Governance::try_from(
                    self.properties.clone(),
                ) {
                    Ok(gov) => gov,
                    Err(e) => {
                        let error = format!(
                            "Apply, Governance_id is empty but can not convert propierties in governance data: {}",
                            e
                        );
                        error!(TARGET_SUBJECT, error);
                        return Err(ActorError::Functional(error));
                    }
                };

                gov.version += 1;
                let gov_value = match to_value(gov) {
                    Ok(value) => value,
                    Err(e) => {
                        let error = format!(
                            "Apply, can not convert governance data into Value: {}",
                            e
                        );
                        error!(TARGET_SUBJECT, error);
                        return Err(ActorError::Functional(error));
                    }
                };

                self.properties.0 = gov_value;
            }
        }

        let last_event_hash = match event
            .hash_id(event.signature.content_hash.derivator)
        {
            Ok(hash) => hash,
            Err(e) => {
                let error =
                    format!("Apply, can not obtain last event hash id: {}", e);
                error!(TARGET_SUBJECT, error);
                return Err(ActorError::Functional(error));
            }
        };

        self.last_event_hash = last_event_hash;
        self.sn += 1;

        Ok(())
    }
}

impl Storable for Subject {}

#[cfg(test)]
mod tests {

    use std::{
        collections::HashSet,
        time::{Duration, Instant},
    };

    use super::*;

    use crate::{
        FactRequest,
        model::{
            event::Event as KoreEvent,
            request::tests::create_start_request_mock, signature::Signature,
        },
        node::NodeResponse,
        system::tests::create_system,
    };

    async fn create_subject_and_ledger_event(
        system: SystemRef,
        node_keys: KeyPair,
    ) -> (
        ActorRef<Subject>,
        ActorRef<LedgerEvent>,
        Subject,
        Signed<Ledger>,
    ) {
        let node = Node::new(&node_keys).unwrap();
        let node_actor = system.create_root_actor("node", node).await.unwrap();
        let request = create_start_request_mock("issuer", node_keys.clone());
        let event = KoreEvent::from_create_request(
            &request,
            0,
            &Governance::new(node_keys.key_identifier())
                .to_value_wrapper()
                .unwrap(),
            DigestDerivator::Blake3_256,
        )
        .unwrap();
        let ledger = Ledger::from(event.clone());
        let signature_ledger = Signature::new(
            &ledger,
            &node_keys.clone(),
            DigestDerivator::Blake3_256,
        )
        .unwrap();
        let signed_ledger = Signed {
            content: ledger,
            signature: signature_ledger,
        };

        let signature_event =
            Signature::new(&event, &node_keys, DigestDerivator::Blake3_256)
                .unwrap();

        let signed_event = Signed {
            content: event,
            signature: signature_event,
        };

        let subject = Subject::from_event(
            &signed_ledger,
            Governance::new(signed_ledger.signature.signer.clone())
                .to_value_wrapper()
                .unwrap(),
        )
        .unwrap();

        let response = node_actor
            .ask(NodeMessage::CreateNewSubjectLedger(signed_ledger.clone()))
            .await
            .unwrap();

        let NodeResponse::SonWasCreated = response else {
            panic!("Invalid response");
        };

        let subject_actor = system
            .get_actor(&ActorPath::from(format!(
                "user/node/{}",
                subject.subject_id
            )))
            .await
            .unwrap();

        let ledger_event_actor: Option<ActorRef<LedgerEvent>> = system
            .get_actor(&ActorPath::from(format!(
                "user/node/{}/ledger_event",
                subject.subject_id
            )))
            .await;

        let ledger_event_actor = if let Some(actor) = ledger_event_actor {
            actor
        } else {
            panic!("Actor must be in system actor");
        };

        ledger_event_actor
            .ask(LedgerEventMessage::UpdateLastEvent {
                event: Box::new(signed_event),
            })
            .await
            .unwrap();

        let response = subject_actor
            .ask(SubjectMessage::UpdateLedger {
                events: vec![signed_ledger.clone()],
            })
            .await
            .unwrap();

        if let SubjectResponse::UpdateResult(last_sn, _, _) = response {
            assert_eq!(last_sn, 0);
        } else {
            panic!("Invalid response");
        }

        (subject_actor, ledger_event_actor, subject, signed_ledger)
    }

    fn create_n_fact_events(
        mut hash_prev_event: DigestIdentifier,
        n: u64,
        keys: KeyPair,
        subject_id: DigestIdentifier,
        mut subject_properties: Value,
    ) -> Vec<Signed<Ledger>> {
        let mut vec: Vec<Signed<Ledger>> = vec![];

        for i in 0..n {
            let key = KeyPair::Ed25519(Ed25519KeyPair::new())
                .key_identifier()
                .to_string();
            let patch_event_req = json!(
                    [{"op":"add","path": format!("/members/KoreNode{}", i),"value": key},
                    {
                        "op": "add",
                        "path": "/version",
                        "value": i
                    }]
            );

            let event_req = EventRequest::Fact(FactRequest {
                subject_id: subject_id.clone(),
                payload: ValueWrapper(patch_event_req.clone()),
            });

            let signature_event_req =
                Signature::new(&event_req, &keys, DigestDerivator::Blake3_256)
                    .unwrap();

            let signed_event_req = Signed {
                content: event_req,
                signature: signature_event_req,
            };

            let patch_json =
                serde_json::from_value::<Patch>(patch_event_req.clone())
                    .unwrap();
            patch(&mut subject_properties, &patch_json).unwrap();

            let state_hash = ValueWrapper(subject_properties.clone())
                .hash_id(DigestDerivator::Blake3_256)
                .unwrap();

            let ledger = Ledger {
                subject_id: subject_id.clone(),
                event_request: signed_event_req,
                sn: i + 1,
                gov_version: i,
                value: LedgerValue::Patch(ValueWrapper(patch_event_req)),
                state_hash,
                eval_success: Some(true),
                appr_required: true,
                appr_success: Some(true),
                vali_success: true,
                hash_prev_event: hash_prev_event.clone(),
            };

            let signature_ledger =
                Signature::new(&ledger, &keys, DigestDerivator::Blake3_256)
                    .unwrap();

            let signed_ledger = Signed {
                content: ledger,
                signature: signature_ledger,
            };

            hash_prev_event =
                signed_ledger.hash_id(DigestDerivator::Blake3_256).unwrap();
            vec.push(signed_ledger);
        }

        vec
    }

    impl KoreEvent {
        pub fn from_create_request(
            request: &Signed<EventRequest>,
            governance_version: u64,
            init_state: &ValueWrapper,
            derivator: DigestDerivator,
        ) -> Result<Self, Error> {
            let EventRequest::Create(_start_request) = &request.content else {
                panic!("Invalid Event Request")
            };

            let state_hash = DigestIdentifier::from_serializable_borsh(
                init_state, derivator,
            )
            .map_err(|_| {
                Error::HashID("Error converting state to hash".to_owned())
            })?;

            let subject_id = request.hash_id(derivator).unwrap();

            Ok(KoreEvent {
                subject_id,
                event_request: request.clone(),
                sn: 0,
                gov_version: governance_version,
                value: LedgerValue::Patch(init_state.clone()),
                state_hash,
                eval_success: None,
                appr_required: false,
                hash_prev_event: DigestIdentifier::default(),
                evaluators: None,
                approvers: None,
                appr_success: None,
                vali_success: true,
                validators: HashSet::new(),
            })
        }
    }

    use event::LedgerEventMessage;
    use identity::{
        identifier::derive::digest::DigestDerivator,
        keys::{Ed25519KeyPair, KeyGenerator, KeyPair},
    };
    use rush::SystemRef;
    use serde_json::{Value, json};
    use test_log::test;

    #[test(tokio::test)]
    async fn test_subject() {
        let (system, ..) = create_system().await;
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let node = Node::new(&node_keys).unwrap();
        let node_actor = system.create_root_actor("node", node).await.unwrap();
        let request = create_start_request_mock("issuer", node_keys.clone());
        let event = KoreEvent::from_create_request(
            &request,
            0,
            &Governance::new(node_keys.key_identifier())
                .to_value_wrapper()
                .unwrap(),
            DigestDerivator::Blake3_256,
        )
        .unwrap();
        let ledger = Ledger::from(event);
        let signature =
            Signature::new(&ledger, &node_keys, DigestDerivator::Blake3_256)
                .unwrap();
        let signed_ledger = Signed {
            content: ledger,
            signature,
        };

        let subject = Subject::from_event(
            &signed_ledger,
            Governance::new(signed_ledger.signature.signer.clone())
                .to_value_wrapper()
                .unwrap(),
        )
        .unwrap();

        assert_eq!(subject.namespace, Namespace::from("namespace"));
        let actor_id = subject.subject_id.to_string();
        node_actor
            .ask(NodeMessage::CreateNewSubjectLedger(signed_ledger.clone()))
            .await
            .unwrap();

        let subject_actor = system
            .get_actor::<Subject>(&ActorPath::from(format!(
                "/user/node/{}",
                subject.subject_id
            )))
            .await
            .unwrap();

        let path = subject_actor.path().clone();

        let response = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap();
        if let SubjectResponse::Metadata(metadata) = response {
            assert_eq!(metadata.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }

        subject_actor.ask_stop().await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;

        let subject_actor = system.get_actor::<Subject>(&path).await;
        assert!(subject_actor.is_none());

        let subject_actor = system
            .create_root_actor(&actor_id, Subject::default())
            .await
            .unwrap();

        let response = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap();
        if let SubjectResponse::Metadata(metadata) = response {
            assert_eq!(metadata.namespace, Namespace::from("namespace"));
        } else {
            panic!("Invalid response");
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let value = Governance::new(KeyIdentifier::default())
            .to_value_wrapper()
            .unwrap();
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let request = create_start_request_mock("issuer", node_keys.clone());
        let event = KoreEvent::from_create_request(
            &request,
            0,
            &value,
            DigestDerivator::Blake3_256,
        )
        .unwrap();

        let ledger = Ledger::from(event);

        let signature =
            Signature::new(&ledger, &node_keys, DigestDerivator::Blake3_256)
                .unwrap();
        let signed_ledger = Signed {
            content: ledger,
            signature,
        };

        let subject_a = Subject::from_event(
            &signed_ledger,
            Governance::new(signed_ledger.signature.signer.clone())
                .to_value_wrapper()
                .unwrap(),
        )
        .unwrap();

        let bytes = bincode::serde::encode_to_vec(
            subject_a.clone(),
            bincode::config::standard(),
        )
        .unwrap();
        let (subject_b, _) = bincode::serde::decode_from_slice::<Subject, _>(
            &bytes,
            bincode::config::standard(),
        )
        .unwrap();
        assert_eq!(subject_a.subject_id, subject_b.subject_id);
    }

    #[test(tokio::test)]
    async fn test_get_events() {
        let (system, ..) = create_system().await;
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());

        let (subject_actor, _ledger_event_actor, _subject, _signed_ledger) =
            create_subject_and_ledger_event(system, node_keys.clone()).await;

        let response = subject_actor
            .ask(SubjectMessage::GetLedger { last_sn: 0 })
            .await
            .unwrap();
        if let SubjectResponse::Ledger {
            ledger, last_event, ..
        } = response
        {
            assert!(ledger.len() == 1);
            last_event.unwrap();
        } else {
            panic!("Invalid response");
        }

        let response = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap();

        if let SubjectResponse::Metadata(metadata) = response {
            assert_eq!(metadata.sn, 0);
        } else {
            panic!("Invalid response");
        }
    }

    #[test(tokio::test)]
    async fn test_1000_events() {
        let node_keys = KeyPair::Ed25519(Ed25519KeyPair::new());
        let (system, ..) = create_system().await;

        let (subject_actor, _ledger_event_actor, subject, signed_ledger) =
            create_subject_and_ledger_event(system, node_keys.clone()).await;

        let hash_pre_event =
            signed_ledger.hash_id(DigestDerivator::Blake3_256).unwrap();

        let inicio = Instant::now();
        let response = subject_actor
            .ask(SubjectMessage::UpdateLedger {
                events: create_n_fact_events(
                    hash_pre_event,
                    1000,
                    node_keys,
                    subject.subject_id,
                    subject.properties.0,
                ),
            })
            .await
            .unwrap();
        let duracion = inicio.elapsed();
        println!("El método tardó: {:.2?}", duracion);

        if let SubjectResponse::UpdateResult(last_sn, _, _) = response {
            assert_eq!(last_sn, 1000);
        } else {
            panic!("Invalid response");
        }

        let response = subject_actor
            .ask(SubjectMessage::GetMetadata)
            .await
            .unwrap();

        if let SubjectResponse::Metadata(metadata) = response {
            assert_eq!(metadata.sn, 1000);
        } else {
            panic!("Invalid response");
        }

        let response = subject_actor
            .ask(SubjectMessage::GetGovernance)
            .await
            .unwrap();

        if let SubjectResponse::Governance(gov) = response {
            assert_eq!(gov.version, 1000);
        } else {
            panic!("Invalid response");
        }
    }
}
