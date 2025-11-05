use crate::{
    CreateRequest, Error, EventRequestType, Governance, Node,
    approval::{
        Approval,
        approver::{Approver, VotationType},
    },
    auth::WitnessesAuth,
    config::Config,
    db::Storable,
    distribution::{Distribution, DistributionType},
    evaluation::{
        Evaluation,
        compiler::{Compiler, CompilerMessage},
        evaluator::Evaluator,
        schema::{EvaluationSchema, EvaluationSchemaMessage},
    },
    governance::{
        Schema,
        model::{CreatorQuantity, HashThisRole, ProtocolTypes, RoleTypes},
    },
    helpers::{db::ExternalDB, sink::KoreSink},
    model::{
        HashId, Namespace, ValueWrapper,
        common::{
            delete_relation, emit_fail, get_gov, get_last_event, get_vali_data,
            register_relation, try_to_update, verify_protocols_state,
        },
        event::{Event as KoreEvent, Ledger, LedgerValue, ProtocolsSignatures},
        request::EventRequest,
        signature::Signed,
    },
    node::{
        NodeMessage, TransferSubject,
        nodekey::{NodeKey, NodeKeyMessage, NodeKeyResponse},
        register::{
            Register, RegisterDataGov, RegisterDataSubj, RegisterMessage,
        },
        transfer,
    },
    subject::Subject,
    update::TransferResponse,
    validation::{
        Validation,
        proof::ValidationProof,
        schema::{ValidationSchema, ValidationSchemaMessage},
        validator::Validator,
    },
};

use event::LedgerEvent;
use identity::identifier::{
    DigestIdentifier, KeyIdentifier, derive::digest::DigestDerivator,
};
use rush::{
    Actor, ActorContext, ActorError, ActorPath, ActorRef, ChildAction, Event,
    Handler, Message, Response, Sink,
};

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use json_patch::{Patch, patch};
use rush::{
    FullPersistence, PersistentActor, Store, StoreCommand, StoreResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use sinkdata::{SinkData, SinkDataMessage};
use tracing::{error, warn};
use transfer::{TransferRegister, TransferRegisterMessage};
use validata::ValiData;

use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTrackerData {
    pub create_req: CreateRequest,
    pub subject_id: DigestIdentifier,
    pub creator: KeyIdentifier,
    pub genesis_gov_version: u64,
    pub value: ValueWrapper,
}

impl Subject for Tracker {}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct Tracker {
    /// The identifier of the subject.
    pub subject_id: DigestIdentifier,
    /// The identifier of the governance that drives this subject.
    pub governance_id: DigestIdentifier,
    /// The version of the governance contract that created the subject.
    pub genesis_gov_version: u64,
    /// The namespace of the subject.
    pub namespace: Namespace,
    /// The identifier of the schema_id used to validate the subject.
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
    /// The current status of the subject.
    pub properties: ValueWrapper,
}

impl Tracker {
    pub fn new(data: CreateTrackerData) -> Self {
        Tracker {
            name: data.create_req.name,
            description: data.create_req.description,
            subject_id: data.subject_id,
            governance_id: data.create_req.governance_id,
            genesis_gov_version: data.genesis_gov_version,
            namespace: data.create_req.namespace,
            schema_id: data.create_req.schema_id,
            owner: data.creator.clone(),
            new_owner: None,
            creator: data.creator,
            last_event_hash: DigestIdentifier::default(),
            active: true,
            sn: 0,
            properties: data.value,
        }
    }

    /// Creates a new `Tracker` from an create event.
    ///
    /// # Arguments
    ///
    /// * `event` - The event.
    /// * `derivator` - The key derivator.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `Tracker` or an `Error`.
    ///
    /// # Errors
    ///
    /// An error is returned if the event is invalid.
    ///
    pub fn from_create_request(
        subject_id: DigestIdentifier,
        request: CreateRequest,
        gov_version: u64,
        owner: KeyIdentifier,
        properties: ValueWrapper,
    ) -> Self {
        Tracker {
            name: request.name.clone(),
            description: request.description.clone(),
            subject_id: subject_id,
            governance_id: request.governance_id.clone(),
            genesis_gov_version: gov_version,
            namespace: request.namespace.clone(),
            schema_id: request.schema_id.clone(),
            last_event_hash: DigestIdentifier::default(),
            owner: owner.clone(),
            new_owner: None,
            creator: owner,
            active: true,
            sn: 0,
            properties,
        }
    }

    /// Returns subject metadata.
    ///
    /// # Returns
    ///
    /// The subject metadata.
    ///
    fn get_metadata(&self) -> Metadata {
        Metadata {
            description: self.description.clone(),
            name: self.name.clone(),
            creator: self.creator.clone(),
            genesis_gov_version: self.genesis_gov_version,
            last_event_hash: self.last_event_hash.clone(),
            subject_id: self.subject_id.clone(),
            governance_id: self.governance_id.clone(),
            schema_id: self.schema_id.clone(),
            namespace: self.namespace.clone(),
            properties: self.properties.clone(),
            owner: self.owner.clone(),
            new_owner: self.new_owner.clone(),
            active: self.active,
            sn: self.sn,
        }
    }

    async fn build_childs_all_schemas(
        &self,
        ctx: &mut ActorContext<Tracker>,
        our_key: KeyIdentifier,
    ) -> Result<(), ActorError> {
        let owner = our_key == self.owner;
        let new_owner = self.new_owner.is_some();
        let i_new_owner = self.new_owner == Some(our_key.clone());

        if new_owner {
            if i_new_owner {
                Self::up_owner_not_gov(ctx, &our_key).await?;
            }
        } else if owner {
            Self::up_owner_not_gov(ctx, &our_key).await?;
        }

        Ok(())
    }

    async fn up_owner_not_gov(
        ctx: &mut ActorContext<Tracker>,
        our_key: &KeyIdentifier,
    ) -> Result<(), ActorError> {
        let validation = Validation::new(our_key.clone());
        ctx.create_child("validation", validation).await?;

        let evaluation = Evaluation::new(our_key.clone());
        ctx.create_child("evaluation", evaluation).await?;

        let distribution =
            Distribution::new(our_key.clone(), DistributionType::Tracker);
        ctx.create_child("distribution", distribution).await?;

        Ok(())
    }

    async fn down_owner_not_gov(
        ctx: &mut ActorContext<Tracker>,
    ) -> Result<(), ActorError> {
        let actor: Option<ActorRef<Validation>> =
            ctx.get_child("validation").await;
        if let Some(actor) = actor {
            actor.ask_stop().await?;
        } else {
            return Err(ActorError::NotFound(ActorPath::from(format!(
                "{}/validation",
                ctx.path()
            ))));
        }

        let actor: Option<ActorRef<Evaluation>> =
            ctx.get_child("evaluation").await;
        if let Some(actor) = actor {
            actor.ask_stop().await?;
        } else {
            return Err(ActorError::NotFound(ActorPath::from(format!(
                "{}/evaluation",
                ctx.path()
            ))));
        }

        let actor: Option<ActorRef<Distribution>> =
            ctx.get_child("distribution").await;
        if let Some(actor) = actor {
            actor.ask_stop().await?;
        } else {
            return Err(ActorError::NotFound(ActorPath::from(format!(
                "{}/distribution",
                ctx.path()
            ))));
        }

        Ok(())
    }

    async fn get_governance_from_other_subject(
        &self,
        ctx: &mut ActorContext<Tracker>,
    ) -> Result<Governance, ActorError> {
        let governance_path =
            ActorPath::from(format!("/user/node/{}", self.governance_id));

        let governance_actor: Option<ActorRef<Governance>> =
            ctx.system().get_actor(&governance_path).await;

        let response = if let Some(governance_actor) = governance_actor {
            governance_actor.ask(TrackerMessage::GetGovernance).await?
        } else {
            return Err(ActorError::NotFound(governance_path));
        };

        match response {
            TrackerResponse::Governance(gov) => Ok(*gov),
            _ => Err(ActorError::UnexpectedResponse(
                governance_path,
                "TrackerResponse::Governance".to_owned(),
            )),
        }
    }

    async fn event_to_sink(
        &self,
        ctx: &mut ActorContext<Tracker>,
        event: &EventRequest,
        issuer: &str,
    ) -> Result<(), ActorError> {
        let gov_id = if self.governance_id.is_empty() {
            None
        } else {
            Some(self.governance_id.to_string())
        };
        let sub_id = self.subject_id.to_string();
        let owner = self.owner.to_string();
        let schema_id = self.schema_id.clone();

        let event_to_sink = match event.clone() {
            EventRequest::Create(..) => SinkDataMessage::Create {
                governance_id: gov_id,
                subject_id: sub_id,
                owner,
                schema_id,
                namespace: self.namespace.to_string(),
                sn: self.sn,
            },
            EventRequest::Fact(fact_request) => SinkDataMessage::Fact {
                governance_id: gov_id,
                subject_id: sub_id,
                issuer: issuer.to_string(),
                owner,
                payload: fact_request.payload.0.clone(),
                schema_id,
                sn: self.sn,
            },
            EventRequest::Transfer(transfer_request) => {
                SinkDataMessage::Transfer {
                    governance_id: gov_id,
                    subject_id: sub_id,
                    owner,
                    new_owner: transfer_request.new_owner.to_string(),
                    schema_id,
                    sn: self.sn,
                }
            }
            EventRequest::Confirm(..) => SinkDataMessage::Confirm {
                governance_id: gov_id,
                subject_id: sub_id,
                schema_id,
                sn: self.sn,
            },
            EventRequest::Reject(..) => SinkDataMessage::Reject {
                governance_id: gov_id,
                subject_id: sub_id,
                schema_id,
                sn: self.sn,
            },
            EventRequest::EOL(..) => SinkDataMessage::EOL {
                governance_id: gov_id,
                subject_id: sub_id,
                schema_id,
                sn: self.sn,
            },
        };

        publish_sink(ctx, event_to_sink).await
    }

    async fn register_relation(
        &self,
        ctx: &mut ActorContext<Tracker>,
        signer: String,
        max_quantity: CreatorQuantity,
    ) -> Result<(), ActorError> {
        register_relation(
            ctx,
            self.governance_id.to_string(),
            self.schema_id.clone(),
            signer,
            self.subject_id.to_string(),
            self.namespace.to_string(),
            max_quantity,
        )
        .await
    }

    async fn delete_subject(
        &self,
        ctx: &mut ActorContext<Tracker>,
    ) -> Result<(), ActorError> {
        if self.schema_id != "governance" {
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

        Self::delet_subject_from_node(ctx, &self.subject_id.to_string())
            .await?;

        Self::purge_storage(ctx).await?;

        ctx.stop(None).await;

        Ok(())
    }

    async fn verify_new_ledger_events_not_gov(
        &mut self,
        ctx: &mut ActorContext<Tracker>,
        events: &[Signed<Ledger>],
    ) -> Result<(), ActorError> {
        let mut events = events.to_vec();
        let last_ledger = self.get_last_ledger_state(ctx).await?;

        let gov = get_gov(ctx, &self.governance_id.to_string()).await?;

        let Some(max_quantity) = gov.max_creations(
            &events[0].signature.signer,
            &self.schema_id,
            self.namespace.clone(),
        ) else {
            return Err(ActorError::Functional(
                "The number of subjects that can be created has not been found"
                    .to_owned(),
            ));
        };

        let mut last_ledger = if let Some(last_ledger) = last_ledger {
            last_ledger
        } else {
            self.register_relation(
                ctx,
                self.owner.to_string(),
                max_quantity.clone(),
            )
            .await?;

            if let Err(e) =
                self.verify_first_ledger_event(events[0].clone()).await
            {
                self.delete_subject(ctx).await?;
                return Err(ActorError::Functional(e.to_string()));
            }

            self.on_event(events[0].clone(), ctx).await;
            self.register(ctx, true).await?;

            events.remove(0)
        };

        for event in events {
            let last_event_is_ok = match self
                .verify_new_ledger_event(&last_ledger, &event)
                .await
            {
                Ok(last_event_is_ok) => last_event_is_ok,
                Err(e) => {
                    if let Error::Sn = e {
                        // El evento que estamos aplicando no es el siguiente.
                        continue;
                    } else {
                        return Err(ActorError::Functional(e.to_string()));
                    }
                }
            };

            if last_event_is_ok {
                match event.content.event_request.content.clone() {
                    EventRequest::Transfer(transfer_request) => {
                        Tracker::new_transfer_subject(
                            ctx,
                            self.name.clone(),
                            &transfer_request.subject_id.to_string(),
                            &transfer_request.new_owner.to_string(),
                            &self.owner.to_string(),
                        )
                        .await?;
                    }
                    EventRequest::Reject(reject_request) => {
                        Tracker::reject_transfer_subject(
                            ctx,
                            &reject_request.subject_id.to_string(),
                        )
                        .await?;
                    }
                    EventRequest::Confirm(confirm_request) => {
                        self.register_relation(
                            ctx,
                            event.signature.signer.to_string(),
                            max_quantity.clone(),
                        )
                        .await?;

                        Tracker::change_node_subject(
                            ctx,
                            &confirm_request.subject_id.to_string(),
                            &event.signature.signer.to_string(),
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

                        Tracker::transfer_register(
                            ctx,
                            &self.subject_id.to_string(),
                            event.signature.signer.clone(),
                            self.owner.clone(),
                        )
                        .await?;
                    }
                    EventRequest::EOL(_eolrequest) => {
                        self.register(ctx, false).await?
                    }
                    _ => {}
                };

                self.event_to_sink(
                    ctx,
                    &event.content.event_request.content,
                    &event.content.event_request.signature.signer.to_string(),
                )
                .await?;
            }

            // Aplicar evento.
            self.on_event(event.clone(), ctx).await;

            // Acutalizar último evento.
            last_ledger = event.clone();
        }

        Ok(())
    }

    async fn verify_ledger_events(
        &mut self,
        ctx: &mut ActorContext<Tracker>,
        events: &[Signed<Ledger>],
    ) -> Result<(), ActorError> {
        let our_key = self.get_node_key(ctx).await?;
        let current_sn = self.sn;
        let current_new_owner_some = self.new_owner.is_some();
        let i_current_new_owner =
            self.new_owner.clone() == Some(our_key.clone());
        let current_owner = self.owner.clone();

        if let Err(e) = self.verify_new_ledger_events_not_gov(ctx, events).await
        {
            if let ActorError::Functional(error) = e.clone() {
                warn!(TARGET_SUBJECT, "Error verifying new events: {}", error);

                // Falló en la creación
                if self.sn == 0 {
                    return Err(e);
                }
            } else {
                error!(TARGET_SUBJECT, "Error verifying new events {}", e);
                return Err(e);
            }
        };

        if current_sn < self.sn {
            if !self.active && current_owner == our_key {
                Self::down_owner_not_gov(ctx).await?;
            }

            let i_new_owner = self.new_owner.clone() == Some(our_key.clone());

            // Si antes no eramos el new owner y ahora somos el new owner.
            if !i_current_new_owner && i_new_owner && current_owner != our_key {
                Self::up_owner_not_gov(ctx, &our_key).await?;
            }

            // Si cambió el dueño
            if current_owner != self.owner {
                // Si ahora somos el dueño pero no eramos new owner.
                if self.owner == our_key && !i_current_new_owner {
                    Self::up_owner_not_gov(ctx, &our_key).await?;
                } else if current_owner == our_key && !i_new_owner {
                    Self::down_owner_not_gov(ctx).await?;
                }
            } else if i_current_new_owner && !i_new_owner {
                Self::down_owner_not_gov(ctx).await?;
            }
        }

        if current_sn < self.sn || current_sn == 0 {
            Self::publish_sink(
                ctx,
                SinkDataMessage::UpdateState(Box::new(self.get_metadata())),
            )
            .await?;

            Self::update_subject_node(
                ctx,
                &self.subject_id.to_string(),
                self.sn,
            )
            .await?;
        }

        Ok(())
    }
}
