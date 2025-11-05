//! # Governance module.
//!

use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;
use identity::identifier::{DigestIdentifier, KeyIdentifier};
use rush::{
    Actor, ActorContext, ActorError, ActorPath, ActorRef, ChildAction,
    FullPersistence, Handler, Message, PersistentActor, Response, Sink,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, warn};

use crate::{
    CreateRequest, Error, EventRequestType,
    approval::{
        Approval,
        approver::{Approver, VotationType},
    },
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
        data::GovernanceData,
        model::{HashThisRole, ProtocolTypes, RoleTypes, Schema},
    },
    helpers::{db::ExternalDB, sink::KoreSink},
    model::{
        HashId, ValueWrapper,
        common::{emit_fail, get_last_storage, get_node_key, purge_storage},
        event::{Event as KoreEvent, Ledger, ProtocolsSignatures},
        request::EventRequest,
        signature::Signed,
    },
    node::register::{RegisterDataGov, RegisterMessage},
    subject::{
        CreateSubjectData, Metadata, Subject, SubjectMetadata, event::LedgerEvent, sinkdata::{SinkData, SinkDataMessage}, validata::ValiData
    },
    validation::{
        Validation,
        proof::ValidationProof,
        schema::{ValidationSchema, ValidationSchemaMessage},
        validator::Validator,
    },
};

pub mod data;
pub mod event;
pub mod model;

const TARGET_GOVERNANCE: &str = "Kore-Governance";

impl Subject for Governance {}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct Governance {
    pub subject_metadata: SubjectMetadata,
    /// The current status of the subject.
    pub properties: GovernanceData,
}

impl Governance {
    pub fn new(data: CreateSubjectData) -> Self {
        Governance {
            subject_metadata: SubjectMetadata::new(&data),
            properties: GovernanceData::new(data.creator),
        }
    }

    pub fn from_create_request(
        subject_id: DigestIdentifier,
        request: CreateRequest,
        owner: KeyIdentifier,
    ) -> Self {
        Governance {
            subject_metadata: SubjectMetadata::from_create_request(
                subject_id,
                request,
                owner.clone(),
            ),
            properties: GovernanceData::new(owner),
        }
    }

    async fn build_childs_governance(
        &self,
        ctx: &mut ActorContext<Governance>,
        our_key: KeyIdentifier,
        ext_db: ExternalDB,
    ) -> Result<(), ActorError> {
        let owner = our_key == self.subject_metadata.owner;
        let new_owner = self.subject_metadata.new_owner.is_some();
        let i_new_owner =
            self.subject_metadata.new_owner == Some(our_key.clone());

        if new_owner {
            if i_new_owner {
                Self::up_owner(
                    ctx,
                    our_key.clone(),
                    self.subject_metadata.subject_id.clone(),
                    ext_db,
                )
                .await?;
            } else {
                Self::up_not_owner(
                    ctx,
                    self.properties.clone(),
                    our_key.clone(),
                    ext_db,
                    self.subject_metadata.subject_id.clone(),
                )
                .await?;
            }
        } else if owner {
            Self::up_owner(
                ctx,
                our_key.clone(),
                self.subject_metadata.subject_id.clone(),
                ext_db,
            )
            .await?;
        } else {
            Self::up_not_owner(
                ctx,
                self.properties.clone(),
                our_key.clone(),
                ext_db,
                self.subject_metadata.subject_id.clone(),
            )
            .await?;
        }

        let schemas =
            self.properties.schemas(ProtocolTypes::Evaluation, &our_key);
        Self::up_compilers_schemas(
            ctx,
            schemas,
            self.subject_metadata.subject_id.clone(),
        )
        .await?;

        let new_creators =
            self.properties.subjects_schemas_rol_namespace(&our_key);

        for creators in new_creators {
            if let Some(eval_user) = creators.evaluation {
                let eval_actor =
                    EvaluationSchema::new(eval_user, self.properties.version);
                ctx.create_child(
                    &format!("{}_evaluation", creators.schema_id),
                    eval_actor,
                )
                .await?;
            }

            if let Some(val_user) = creators.validation {
                let actor =
                    ValidationSchema::new(val_user, self.properties.version);
                ctx.create_child(
                    &format!("{}_validation", creators.schema_id),
                    actor,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn up_not_owner(
        ctx: &mut ActorContext<Self>,
        gov: GovernanceData,
        our_key: KeyIdentifier,
        ext_db: ExternalDB,
        subject_id: DigestIdentifier,
    ) -> Result<(), ActorError> {
        if gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Validator,
        }) {
            // If we are a validator
            let validator = Validator::default();
            ctx.create_child("validator", validator).await?;
        }

        if gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Evaluator,
        }) {
            // If we are a evaluator
            let evaluator = Evaluator::default();
            ctx.create_child("evaluator", evaluator).await?;
        }

        if gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Approver,
        }) {
            let Some(config): Option<Config> =
                ctx.system().get_helper("config").await
            else {
                return Err(ActorError::NotHelper("config".to_owned()));
            };

            // If we are a approver
            let approver = Approver::new(
                String::default(),
                0,
                our_key.clone(),
                subject_id.to_string(),
                VotationType::from(config.always_accept),
            );
            let approver_actor = ctx.create_child("approver", approver).await?;

            let sink =
                Sink::new(approver_actor.subscribe(), ext_db.get_approver());
            ctx.system().run_sink(sink).await;
        }

        Ok(())
    }

    async fn up_owner(
        ctx: &mut ActorContext<Governance>,
        our_key: KeyIdentifier,
        subject_id: DigestIdentifier,
        ext_db: ExternalDB,
    ) -> Result<(), ActorError> {
        let Some(config): Option<Config> =
            ctx.system().get_helper("config").await
        else {
            return Err(ActorError::NotHelper("config".to_owned()));
        };

        let validation = Validation::new(our_key.clone());
        ctx.create_child("validation", validation).await?;

        let evaluation = Evaluation::new(our_key.clone());
        ctx.create_child("evaluation", evaluation).await?;

        let approval = Approval::new(our_key.clone());
        ctx.create_child("approval", approval).await?;

        let approver = Approver::new(
            String::default(),
            0,
            our_key.clone(),
            subject_id.to_string(),
            VotationType::from(config.always_accept),
        );
        let approver_actor = ctx.create_child("approver", approver).await?;

        let sink = Sink::new(approver_actor.subscribe(), ext_db.get_approver());
        ctx.system().run_sink(sink).await;

        let distribution =
            Distribution::new(our_key.clone(), DistributionType::Subject);
        ctx.create_child("distribution", distribution).await?;

        Ok(())
    }

    async fn up_compilers_schemas(
        ctx: &mut ActorContext<Governance>,
        schemas: BTreeMap<String, Schema>,
        subject_id: DigestIdentifier,
    ) -> Result<(), ActorError> {
        let Some(config): Option<Config> =
            ctx.system().get_helper("config").await
        else {
            return Err(ActorError::NotHelper("config".to_owned()));
        };

        for (id, schema) in schemas {
            let actor_name = format!("{}_compiler", id);

            let compiler =
                if let Some(compiler) = ctx.get_child(&actor_name).await {
                    compiler
                } else {
                    ctx.create_child(&actor_name, Compiler::default()).await?
                };

            compiler
                .tell(CompilerMessage::Compile {
                    contract_name: format!("{}_{}", subject_id, id),
                    contract: schema.contract.clone(),
                    initial_value: schema.initial_value.clone(),
                    contract_path: format!(
                        "{}/contracts/{}_{}",
                        config.contracts_dir, subject_id, id
                    ),
                })
                .await?;
        }

        Ok(())
    }

    async fn down_not_owner(
        ctx: &mut ActorContext<Self>,
        gov: GovernanceData,
        our_key: KeyIdentifier,
    ) -> Result<(), ActorError> {
        if gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Validator,
        }) {
            let actor: Option<ActorRef<Validator>> =
                ctx.get_child("validator").await;
            if let Some(actor) = actor {
                actor.ask_stop().await?;
            } else {
                return Err(ActorError::NotFound(ActorPath::from(format!(
                    "{}/validator",
                    ctx.path()
                ))));
            }
        }

        if gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Evaluator,
        }) {
            let actor: Option<ActorRef<Evaluator>> =
                ctx.get_child("evaluator").await;
            if let Some(actor) = actor {
                actor.ask_stop().await?;
            } else {
                return Err(ActorError::NotFound(ActorPath::from(format!(
                    "{}/evaluator",
                    ctx.path()
                ))));
            }
        }

        if gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Approver,
        }) {
            let actor: Option<ActorRef<Approver>> =
                ctx.get_child("approver").await;
            if let Some(actor) = actor {
                actor.ask_stop().await?;
            } else {
                return Err(ActorError::NotFound(ActorPath::from(format!(
                    "{}/approver",
                    ctx.path()
                ))));
            }
        }

        Ok(())
    }

    async fn up_down_not_owner(
        ctx: &mut ActorContext<Self>,
        new_gov: GovernanceData,
        old_gov: GovernanceData,
        our_key: KeyIdentifier,
        ext_db: ExternalDB,
        subject_id: DigestIdentifier,
    ) -> Result<(), ActorError> {
        let old_val = old_gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Validator,
        });

        let new_val = new_gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Validator,
        });

        match (old_val, new_val) {
            (true, false) => {
                let actor: Option<ActorRef<Validator>> =
                    ctx.get_child("validator").await;
                if let Some(actor) = actor {
                    actor.ask_stop().await?;
                } else {
                    return Err(ActorError::NotFound(ActorPath::from(
                        format!("{}/validator", ctx.path()),
                    )));
                }
            }
            (false, true) => {
                let validator = Validator::default();
                ctx.create_child("validator", validator).await?;
            }
            _ => {}
        };

        let old_eval = old_gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Evaluator,
        });

        let new_eval = new_gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Evaluator,
        });

        match (old_eval, new_eval) {
            (true, false) => {
                let actor: Option<ActorRef<Evaluator>> =
                    ctx.get_child("evaluator").await;
                if let Some(actor) = actor {
                    actor.ask_stop().await?;
                } else {
                    return Err(ActorError::NotFound(ActorPath::from(
                        format!("{}/evaluator", ctx.path()),
                    )));
                }
            }
            (false, true) => {
                let evaluator = Evaluator::default();
                ctx.create_child("evaluator", evaluator).await?;
            }
            _ => {}
        };

        let old_appr = old_gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Approver,
        });

        let new_appr = new_gov.has_this_role(HashThisRole::Gov {
            who: our_key.clone(),
            role: RoleTypes::Approver,
        });

        match (old_appr, new_appr) {
            (true, false) => {
                let actor: Option<ActorRef<Approver>> =
                    ctx.get_child("approver").await;
                if let Some(actor) = actor {
                    actor.ask_stop().await?;
                } else {
                    return Err(ActorError::NotFound(ActorPath::from(
                        format!("{}/approver", ctx.path()),
                    )));
                }
            }
            (false, true) => {
                let Some(config): Option<Config> =
                    ctx.system().get_helper("config").await
                else {
                    return Err(ActorError::NotHelper("config".to_owned()));
                };

                // If we are a approver
                let approver = Approver::new(
                    String::default(),
                    0,
                    our_key.clone(),
                    subject_id.to_string(),
                    VotationType::from(config.always_accept),
                );
                let approver_actor =
                    ctx.create_child("approver", approver).await?;

                let sink = Sink::new(
                    approver_actor.subscribe(),
                    ext_db.get_approver(),
                );
                ctx.system().run_sink(sink).await;
            }
            _ => {}
        };

        Ok(())
    }

    async fn down_owner(
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        let actor: Option<ActorRef<Validation>> =
            ctx.get_child("validation").await;
        if let Some(actor) = actor {
            actor.ask_stop().await?;
        } else {
            let e = ActorError::NotFound(ActorPath::from(format!(
                "{}/validation",
                ctx.path()
            )));
            return Err(emit_fail(ctx, e).await);
        }

        let actor: Option<ActorRef<Evaluation>> =
            ctx.get_child("evaluation").await;
        if let Some(actor) = actor {
            actor.ask_stop().await?;
        } else {
            let e = ActorError::NotFound(ActorPath::from(format!(
                "{}/evaluation",
                ctx.path()
            )));
            return Err(emit_fail(ctx, e).await);
        }

        let actor: Option<ActorRef<Approval>> = ctx.get_child("approval").await;
        if let Some(actor) = actor {
            actor.ask_stop().await?;
        } else {
            let e = ActorError::NotFound(ActorPath::from(format!(
                "{}/approval",
                ctx.path()
            )));
            return Err(emit_fail(ctx, e).await);
        }

        let actor: Option<ActorRef<Approver>> = ctx.get_child("approver").await;
        if let Some(actor) = actor {
            actor.ask_stop().await?;
        } else {
            let e = ActorError::NotFound(ActorPath::from(format!(
                "{}/approver",
                ctx.path()
            )));
            return Err(emit_fail(ctx, e).await);
        }

        let actor: Option<ActorRef<Distribution>> =
            ctx.get_child("distribution").await;
        if let Some(actor) = actor {
            actor.ask_stop().await?;
        } else {
            let e = ActorError::NotFound(ActorPath::from(format!(
                "{}/distribution",
                ctx.path()
            )));
            return Err(emit_fail(ctx, e).await);
        }

        Ok(())
    }

    async fn down_compilers_schemas(
        ctx: &mut ActorContext<Self>,
        schemas: Vec<String>,
    ) -> Result<(), ActorError> {
        for schema in schemas {
            let actor: Option<ActorRef<Compiler>> =
                ctx.get_child(&format!("{}_compiler", schema)).await;
            if let Some(actor) = actor {
                actor.ask_stop().await?;
            } else {
                return Err(ActorError::NotFound(ActorPath::from(format!(
                    "{}/{}_compiler",
                    ctx.path(),
                    schema
                ))));
            }
        }

        Ok(())
    }

    async fn down_schemas(
        ctx: &mut ActorContext<Self>,
        old_schemas_eval: Vec<String>,
        old_schemas_val: Vec<String>,
    ) -> Result<(), ActorError> {
        for schema in old_schemas_eval {
            let actor: Option<ActorRef<EvaluationSchema>> =
                ctx.get_child(&format!("{}_evaluation", schema)).await;
            if let Some(actor) = actor {
                actor.ask_stop().await?;
            } else {
                return Err(ActorError::NotFound(ActorPath::from(format!(
                    "{}/{}_evaluation",
                    ctx.path(),
                    schema
                ))));
            }
        }

        for schema_id in old_schemas_val {
            let actor: Option<ActorRef<ValidationSchema>> =
                ctx.get_child(&format!("{}_validation", schema_id)).await;
            if let Some(actor) = actor {
                actor.ask_stop().await?;
            } else {
                return Err(ActorError::NotFound(ActorPath::from(format!(
                    "{}/{}_validation",
                    ctx.path(),
                    schema_id
                ))));
            }
        }

        Ok(())
    }

    async fn compile_schemas(
        ctx: &mut ActorContext<Self>,
        schemas: HashMap<String, Schema>,
        subject_id: DigestIdentifier,
    ) -> Result<(), ActorError> {
        let Some(config): Option<Config> =
            ctx.system().get_helper("config").await
        else {
            return Err(ActorError::NotHelper("config".to_owned()));
        };

        for (id, schema) in schemas {
            let actor: Option<ActorRef<Compiler>> =
                ctx.get_child(&format!("{}_compiler", id)).await;
            if let Some(actor) = actor {
                actor
                    .tell(CompilerMessage::Compile {
                        contract_name: format!("{}_{}", subject_id, id),
                        contract: schema.contract.clone(),
                        initial_value: schema.initial_value.clone(),
                        contract_path: format!(
                            "{}/contracts/{}_{}",
                            config.contracts_dir, subject_id, id
                        ),
                    })
                    .await?;
            } else {
                return Err(ActorError::NotFound(ActorPath::from(format!(
                    "{}/{}_compiler",
                    ctx.path(),
                    id
                ))));
            }
        }

        Ok(())
    }

    async fn event_to_sink(
        &self,
        ctx: &mut ActorContext<Governance>,
        event: &EventRequest,
        issuer: &str,
    ) -> Result<(), ActorError> {
        let governance_id = None;
        let subject_id = self.subject_metadata.subject_id.to_string();
        let owner = self.subject_metadata.owner.to_string();
        let schema_id = self.subject_metadata.schema_id.to_string();
        let sn = self.subject_metadata.sn;

        let event_to_sink = match event.clone() {
            EventRequest::Create(..) => SinkDataMessage::Create {
                governance_id,
                subject_id,
                owner,
                schema_id,
                namespace: String::default(),
                sn,
            },
            EventRequest::Fact(fact_request) => SinkDataMessage::Fact {
                governance_id,
                subject_id,
                issuer: issuer.to_string(),
                owner,
                payload: fact_request.payload.0.clone(),
                schema_id,
                sn,
            },
            EventRequest::Transfer(transfer_request) => {
                SinkDataMessage::Transfer {
                    governance_id,
                    subject_id,
                    owner,
                    new_owner: transfer_request.new_owner.to_string(),
                    schema_id,
                    sn,
                }
            }
            EventRequest::Confirm(..) => SinkDataMessage::Confirm {
                governance_id,
                subject_id,
                schema_id,
                sn,
            },
            EventRequest::Reject(..) => SinkDataMessage::Reject {
                governance_id,
                subject_id,
                schema_id,
                sn,
            },
            EventRequest::EOL(..) => SinkDataMessage::EOL {
                governance_id,
                subject_id,
                schema_id,
                sn,
            },
        };

        Self::publish_sink(ctx, event_to_sink).await
    }

    async fn verify_ledger_events(
        &mut self,
        ctx: &mut ActorContext<Self>,
        events: &[Signed<Ledger>],
    ) -> Result<(), ActorError> {
        let mut events = events.to_vec();
        let last_ledger = get_last_storage(ctx).await?;

        let mut last_ledger = if let Some(last_ledger) = last_ledger {
            last_ledger
        } else {
            if let Err(e) = Self::verify_create_event(
                self.subject_metadata.owner,
                &events[0].clone(),
                true,
            )
            .await
            {
                self.delete_subject(ctx).await?;
                return Err(ActorError::Functional(e.to_string()));
            }

            self.on_event(events[0].clone(), ctx).await;
            Self::register(
                ctx,
                RegisterMessage::RegisterGov {
                    gov_id: self.subject_metadata.subject_id.to_string(),
                    data: RegisterDataGov {
                        active: true,
                        name: self.subject_metadata.name.clone(),
                        description: self.subject_metadata.description.clone(),
                    },
                },
            )
            .await?;

            self.event_to_sink(
                ctx,
                &events[0].content.event_request.content,
                &events[0].content.event_request.signature.signer.to_string(),
            )
            .await?;

            events.remove(0)
        };

        for event in events {
            let last_event_is_ok = match Self::verify_event(
                &self.subject_metadata,
                &ValueWrapper(json!(self.properties)),
                &last_ledger,
                &event,
            ) {
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
                        Self::new_transfer_subject(
                            ctx,
                            self.subject_metadata.name.clone(),
                            &transfer_request.subject_id.to_string(),
                            &transfer_request.new_owner.to_string(),
                            &self.subject_metadata.owner.to_string(),
                        )
                        .await?
                    }
                    EventRequest::Reject(reject_request) => {
                        Self::reject_transfer_subject(
                            ctx,
                            &reject_request.subject_id.to_string(),
                        )
                        .await?;
                    }
                    EventRequest::Confirm(confirm_request) => {
                        Self::change_node_subject(
                            ctx,
                            &confirm_request.subject_id.to_string(),
                            &event.signature.signer.to_string(),
                            &self.subject_metadata.owner.to_string(),
                        )
                        .await?;
                    }
                    EventRequest::EOL(_eolrequest) => {
                        Self::register(
                            ctx,
                            RegisterMessage::RegisterGov {
                                gov_id: self
                                    .subject_metadata
                                    .subject_id
                                    .to_string(),
                                data: RegisterDataGov {
                                    active: false,
                                    name: self.subject_metadata.name.clone(),
                                    description: self
                                        .subject_metadata
                                        .description
                                        .clone(),
                                },
                            },
                        )
                        .await?;
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

    async fn delete_subject(
        &self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        Self::delet_subject_from_node(
            ctx,
            &self.subject_metadata.subject_id.to_string(),
        )
        .await?;

        purge_storage(ctx).await?;

        // TODO, un actor se debería poder parar así mismo?
        ctx.stop(None).await;

        Ok(())
    }

    async fn manage_ledger_events(
        &mut self,
        ctx: &mut ActorContext<Self>,
        events: &[Signed<Ledger>],
    ) -> Result<(), ActorError> {
        let our_key = get_node_key(ctx).await?;
        let current_sn = self.subject_metadata.sn;
        let current_new_owner_some = self.subject_metadata.new_owner.is_some();
        let i_current_new_owner =
            self.subject_metadata.new_owner.clone() == Some(our_key.clone());
        let current_owner = self.subject_metadata.owner.clone();

        let old_gov = self.properties.clone();

        if let Err(e) = self.verify_ledger_events(ctx, events).await {
            if let ActorError::Functional(error) = e.clone() {
                warn!(
                    TARGET_GOVERNANCE,
                    "Error verifying new events: {}", error
                );

                // Falló en la creación
                if self.subject_metadata.sn == 0 {
                    return Err(e);
                }
                // TODO falló pero pudo aplicar algún evento entonces seguimos.
            } else {
                error!(TARGET_GOVERNANCE, "Error verifying new events {}", e);
                return Err(e);
            }
        };

        if current_sn < self.subject_metadata.sn {
            if !self.subject_metadata.active {
                if current_owner == our_key {
                    Self::down_owner(ctx).await?;
                } else {
                    Self::down_not_owner(ctx, old_gov.clone(), our_key.clone())
                        .await?;
                }

                let old_schemas_eval = old_gov
                    .schemas(ProtocolTypes::Evaluation, &our_key)
                    .iter()
                    .map(|x| x.0.clone())
                    .collect::<Vec<String>>();
                Self::down_compilers_schemas(ctx, old_schemas_eval.clone())
                    .await?;

                let old_schemas_val = old_gov
                    .schemas(ProtocolTypes::Validation, &our_key)
                    .iter()
                    .map(|x| x.0.clone())
                    .collect::<Vec<String>>();

                Self::down_schemas(ctx, old_schemas_eval, old_schemas_val)
                    .await?;
            } else {
                let new_gov = self.properties.clone();

                let Some(ext_db): Option<ExternalDB> =
                    ctx.system().get_helper("ext_db").await
                else {
                    return Err(ActorError::NotHelper("config".to_owned()));
                };

                let new_owner_some = self.subject_metadata.new_owner.is_some();
                let i_new_owner = self.subject_metadata.new_owner.clone()
                    == Some(our_key.clone());
                let mut up_not_owner: bool = false;
                let mut up_owner: bool = false;

                if current_owner == our_key {
                    // Eramos dueños
                    if current_owner != self.subject_metadata.owner {
                        // Ya no somos dueño
                        if !current_new_owner_some && !i_new_owner {
                            // Si antes new owner false
                            up_not_owner = true;
                        } else if current_new_owner_some && i_new_owner {
                            up_owner = true;
                        }
                    } else {
                        // Seguimos siendo dueños
                        if current_new_owner_some && !new_owner_some {
                            up_owner = true;
                        } else if !current_new_owner_some && new_owner_some {
                            up_not_owner = true;
                        }
                    }
                } else {
                    // No eramos dueño
                    if current_owner != self.subject_metadata.owner
                        && self.subject_metadata.owner == our_key
                    {
                        // Ahora Somos dueños
                        if !new_owner_some && !i_current_new_owner {
                            // new owner false
                            up_owner = true;
                        } else if new_owner_some && i_current_new_owner {
                            up_not_owner = true;
                        }
                    } else if i_current_new_owner && !i_new_owner {
                        up_not_owner = true;
                    } else if !i_current_new_owner && i_new_owner {
                        up_owner = true;
                    }
                }

                if up_not_owner {
                    Self::down_owner(ctx).await?;
                    Self::up_not_owner(
                        ctx,
                        new_gov.clone(),
                        our_key.clone(),
                        ext_db.clone(),
                        self.subject_metadata.subject_id.clone(),
                    )
                    .await?;
                } else if up_owner {
                    Self::down_not_owner(ctx, old_gov.clone(), our_key.clone())
                        .await?;
                    Self::up_owner(
                        ctx,
                        our_key.clone(),
                        self.subject_metadata.subject_id.clone(),
                        ext_db.clone(),
                    )
                    .await?;
                }

                // Seguimos sin ser owner ni new owner,
                // pero tenemos que ver si tenemos un rol nuevo.
                if !up_not_owner
                    && !up_owner
                    && our_key != self.subject_metadata.owner
                {
                    Self::up_down_not_owner(
                        ctx,
                        new_gov.clone(),
                        old_gov.clone(),
                        our_key.clone(),
                        ext_db.clone(),
                        self.subject_metadata.subject_id.clone(),
                    )
                    .await?;
                }

                let old_schemas_eval =
                    old_gov.schemas(ProtocolTypes::Evaluation, &our_key);
                let new_schemas_eval =
                    new_gov.schemas(ProtocolTypes::Evaluation, &our_key);

                // Bajamos los compilers que ya no soy evaluador
                let down = old_schemas_eval
                    .clone()
                    .iter()
                    .filter(|x| !new_schemas_eval.contains_key(x.0))
                    .map(|x| x.0.clone())
                    .collect();
                Self::down_compilers_schemas(ctx, down).await?;

                // Subimos los compilers que soy nuevo evaluador
                let up = new_schemas_eval
                    .clone()
                    .iter()
                    .filter(|x| !old_schemas_eval.contains_key(x.0))
                    .map(|x| (x.0.clone(), x.1.clone()))
                    .collect();
                Self::up_compilers_schemas(
                    ctx,
                    up,
                    self.subject_metadata.subject_id.clone(),
                )
                .await?;

                // Compilo los nuevos contratos en el caso de que hayan sido modificados, sino no afecta.
                let current = new_schemas_eval
                    .clone()
                    .iter()
                    .filter(|x| old_schemas_eval.contains_key(x.0))
                    .map(|x| (x.0.clone(), x.1.clone()))
                    .collect();
                Self::compile_schemas(
                    ctx,
                    current,
                    self.subject_metadata.subject_id.clone(),
                )
                .await?;

                let mut old_schemas_eval = old_schemas_eval
                    .iter()
                    .map(|x| x.0.clone())
                    .collect::<Vec<String>>();
                let mut old_schemas_val = old_gov
                    .schemas(ProtocolTypes::Validation, &our_key)
                    .iter()
                    .map(|x| x.0.clone())
                    .collect::<Vec<String>>();

                let new_creators =
                    new_gov.subjects_schemas_rol_namespace(&our_key);

                for creators in new_creators {
                    if let Some(eval_users) = creators.evaluation {
                        let pos = old_schemas_eval
                            .iter()
                            .position(|x| *x == creators.schema_id);

                        if let Some(pos) = pos {
                            old_schemas_eval.remove(pos);
                            let actor: Option<ActorRef<EvaluationSchema>> = ctx
                                .get_child(&format!(
                                    "{}_evaluation",
                                    creators.schema_id
                                ))
                                .await;
                            if let Some(actor) = actor {
                                if let Err(e) = actor.tell(EvaluationSchemaMessage::UpdateEvaluators(eval_users, new_gov.version)).await {
                                        return Err(emit_fail(ctx, e).await);
                                    }
                            } else {
                                let e = ActorError::NotFound(ActorPath::from(
                                    format!(
                                        "{}/{}_evaluation",
                                        ctx.path(),
                                        creators.schema_id
                                    ),
                                ));
                                return Err(emit_fail(ctx, e).await);
                            }
                        } else {
                            let eval_actor = EvaluationSchema::new(
                                eval_users,
                                new_gov.version,
                            );
                            ctx.create_child(
                                &format!("{}_evaluation", creators.schema_id),
                                eval_actor,
                            )
                            .await?;
                        }
                    }

                    if let Some(val_user) = creators.validation {
                        let pos = old_schemas_val
                            .iter()
                            .position(|x| *x == creators.schema_id);
                        if let Some(pos) = pos {
                            old_schemas_val.remove(pos);
                            let actor: Option<ActorRef<ValidationSchema>> = ctx
                                .get_child(&format!(
                                    "{}_validation",
                                    creators.schema_id
                                ))
                                .await;
                            if let Some(actor) = actor {
                                if let Err(e) = actor.tell(ValidationSchemaMessage::UpdateValidators(val_user, new_gov.version)).await {
                                        return Err(emit_fail(ctx, e).await);
                                    }
                            } else {
                                let e = ActorError::NotFound(ActorPath::from(
                                    format!(
                                        "{}/{}_validation",
                                        ctx.path(),
                                        creators.schema_id
                                    ),
                                ));
                                return Err(emit_fail(ctx, e).await);
                            }
                        } else {
                            let actor = ValidationSchema::new(
                                val_user,
                                new_gov.version,
                            );
                            ctx.create_child(
                                &format!("{}_validation", creators.schema_id),
                                actor,
                            )
                            .await?;
                        }
                    }
                }

                Self::down_schemas(ctx, old_schemas_eval, old_schemas_val)
                    .await?;
            }
        }

        if current_sn < self.subject_metadata.sn || current_sn == 0 {
            Self::publish_sink(
                ctx,
                SinkDataMessage::UpdateState(Box::new(Metadata::from(
                    self.clone(),
                ))),
            )
            .await?;

            Self::update_subject_node(
                ctx,
                &self.subject_metadata.subject_id.to_string(),
                self.subject_metadata.sn,
            )
            .await?;
        }

        Ok(())
    }

    async fn create_compilers(
        ctx: &mut ActorContext<Self>,
        compilers: &[String],
    ) -> Result<Vec<String>, ActorError> {
        let mut new_compilers = vec![];

        for compiler in compilers {
            let child: Option<ActorRef<Compiler>> =
                ctx.get_child(&format!("{}_compiler", compiler)).await;
            if child.is_none() {
                new_compilers.push(compiler.clone());

                ctx.create_child(
                    &format!("{}_compiler", compiler),
                    Compiler::default(),
                )
                .await?;
            }
        }

        Ok(new_compilers)
    }
}

#[derive(Debug, Clone)]
pub enum GovernanceMessage {
    CreateCompilers(Vec<String>),
    GetMetadata,
    GetLedger { last_sn: u64 },
    GetLastLedger,
    UpdateLedger { events: Vec<Signed<Ledger>> },
    GetGovernance,
    GetOwner,
}

impl Message for GovernanceMessage {}

#[derive(Debug, Clone)]
pub enum GovernanceResponse {
    Metadata(Box<Metadata>),
    UpdateResult(u64, KeyIdentifier, Option<KeyIdentifier>),
    Ledger {
        ledger: Vec<Signed<Ledger>>,
        event: Box<Option<Signed<KoreEvent>>>,
        proof: Box<Option<ValidationProof>>,
        vali_res: Option<Vec<ProtocolsSignatures>>,
    },
    Governance(Box<GovernanceData>),
    Owner(KeyIdentifier),
    NewCompilers(Vec<String>),
    Ok,
}

impl Response for GovernanceResponse {}

impl Actor for Governance {
    type Event = Signed<Ledger>;
    type Message = GovernanceMessage;
    type Response = GovernanceResponse;

    async fn pre_start(
        &mut self,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), ActorError> {
        self.init_store("governance", None, true, ctx).await?;

        let our_key = get_node_key(ctx).await?;

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

        let ledger_event = LedgerEvent::new(self.subject_metadata.schema_id.is_empty());
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

        if self.subject_metadata.active {
            self.build_childs_governance(ctx, our_key.clone(), ext_db.clone())
                .await?;
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
impl Handler<Governance> for Governance {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: GovernanceMessage,
        ctx: &mut ActorContext<Self>,
    ) -> Result<GovernanceResponse, ActorError> {
        /*
        Migrar al node
        GovernanceMessage::UpdateTransfer(res) => {
            match res {
                TransferResponse::Confirm => {
                    let Some(new_owner) = self.subject_metadata.new_owner.clone() else {
                        let e = "Can not obtain new_owner";
                        error!(TARGET_GOVERNANCE, "Confirm, {}", e);
                        return Err(ActorError::Functional(e.to_owned()));
                    };

                    Self::change_node_subject(
                        ctx,
                        &self.subject_metadata.subject_id.to_string(),
                        &new_owner.to_string(),
                        &self.subject_metadata.owner.to_string(),
                    )
                    .await?;

                    delete_relation(
                        ctx,
                        self.governance_id.to_string(),
                        self.schema_id.clone(),
                        self.subject_metadata.owner.to_string(),
                        self.subject_id.to_string(),
                        self.namespace.to_string(),
                    )
                    .await?;
                }
                TransferResponse::Reject => {
                    Self::reject_transfer_subject(
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

            Ok(GovernanceResponse::Ok)
        }
        */
        /*
            // Migrar al node
            GovernanceMessage::DeleteSubject => {
                self.delete_subject(ctx).await?;
                Ok(GovernanceResponse::Ok)
            }
        */
        match msg {
            GovernanceMessage::CreateCompilers(compilers) => {
                let new_compilers =
                    match Self::create_compilers(ctx, &compilers).await {
                        Ok(new_compilers) => new_compilers,
                        Err(e) => {
                            warn!(
                                TARGET_GOVERNANCE,
                                "CreateCompilers, can not create compilers: {}",
                                e
                            );
                            return Err(e);
                        }
                    };
                Ok(GovernanceResponse::NewCompilers(new_compilers))
            }
            GovernanceMessage::GetLedger { last_sn } => {
                let (ledger, event, proof, vali_res) = Self::get_ledger_data(
                    ctx,
                    &self.subject_metadata.subject_id.to_string(),
                    last_sn,
                    100,
                )
                .await?;
                Ok(GovernanceResponse::Ledger {
                    ledger,
                    event: Box::new(event),
                    proof: Box::new(proof),
                    vali_res,
                })
            }
            GovernanceMessage::GetLastLedger => {
                let (ledger, event, proof, vali_res) = Self::get_ledger_data(
                    ctx,
                    &self.subject_metadata.subject_id.to_string(),
                    self.subject_metadata.sn,
                    100,
                )
                .await?;
                Ok(GovernanceResponse::Ledger {
                    ledger,
                    event: Box::new(event),
                    proof: Box::new(proof),
                    vali_res,
                })
            }
            GovernanceMessage::GetOwner => Ok(GovernanceResponse::Owner(
                self.subject_metadata.owner.clone(),
            )),
            GovernanceMessage::GetMetadata => Ok(GovernanceResponse::Metadata(
                Box::new(Metadata::from(self.clone())),
            )),
            GovernanceMessage::UpdateLedger { events } => {
                if let Err(e) =
                    self.manage_ledger_events(ctx, events.as_slice()).await
                {
                    warn!(
                        TARGET_GOVERNANCE,
                        "UpdateLedger, can not verify new events: {}", e
                    );
                    return Err(e);
                };
                Ok(GovernanceResponse::UpdateResult(
                    self.subject_metadata.sn,
                    self.subject_metadata.owner.clone(),
                    self.subject_metadata.new_owner.clone(),
                ))
            }
            GovernanceMessage::GetGovernance => {
                Ok(GovernanceResponse::Governance(Box::new(
                    self.properties.clone(),
                )))
            }
        }
    }

    async fn on_event(
        &mut self,
        event: Signed<Ledger>,
        ctx: &mut ActorContext<Self>,
    ) {
        if let Err(e) = self.persist(&event, ctx).await {
            error!(
                TARGET_GOVERNANCE,
                "OnEvent, can not persist information: {}", e
            );
            emit_fail(ctx, e).await;
        };

        if let Err(e) = ctx.publish_event(event).await {
            error!(
                TARGET_GOVERNANCE,
                "PublishEvent, can not publish event: {}", e
            );
            emit_fail(ctx, e).await;
        }
    }

    async fn on_child_fault(
        &mut self,
        error: ActorError,
        ctx: &mut ActorContext<Self>,
    ) -> ChildAction {
        error!(TARGET_GOVERNANCE, "OnChildFault, {}", error);
        emit_fail(ctx, error).await;
        ChildAction::Stop
    }
}

#[async_trait]
impl PersistentActor for Governance {
    type Persistence = FullPersistence;

    fn apply(&mut self, event: &Self::Event) -> Result<(), ActorError> {
        let valid_event = match Self::verify_protocols_state(
            EventRequestType::from(event.content.event_request.content.clone()),
            event.content.eval_success,
            event.content.appr_success,
            event.content.appr_required,
            event.content.vali_success,
            true,
        ) {
            Ok(is_ok) => is_ok,
            Err(e) => {
                let error =
                    format!("Apply, can not verify protocols state: {}", e);
                error!(TARGET_GOVERNANCE, error);
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
                            error!(TARGET_GOVERNANCE, error);
                            return Err(ActorError::Functional(error));
                        }
                    };

                    self.subject_metadata.last_event_hash = last_event_hash;
                    return Ok(());
                }
                EventRequest::Fact(_fact_request) => {
                    self.apply_patch(event.content.value.clone())?;
                }
                EventRequest::Transfer(transfer_request) => {
                    self.subject_metadata.new_owner =
                        Some(transfer_request.new_owner.clone());
                }
                EventRequest::Confirm(_confirm_request) => {
                    self.apply_patch(event.content.value.clone())?;

                    let Some(new_owner) =
                        self.subject_metadata.new_owner.clone()
                    else {
                        let error = "In confirm event was succefully but new owner is empty:";
                        error!(TARGET_GOVERNANCE, error);
                        return Err(ActorError::Functional(error.to_owned()));
                    };

                    self.subject_metadata.owner = new_owner;
                    self.subject_metadata.new_owner = None;
                }
                EventRequest::Reject(_reject_request) => {
                    self.subject_metadata.new_owner = None;
                }
                EventRequest::EOL(_eolrequest) => {
                    self.subject_metadata.active = false
                }
            }

            self.properties.version += 1;
        }

        let last_event_hash = match event
            .hash_id(event.signature.content_hash.derivator)
        {
            Ok(hash) => hash,
            Err(e) => {
                let error =
                    format!("Apply, can not obtain last event hash id: {}", e);
                error!(TARGET_GOVERNANCE, error);
                return Err(ActorError::Functional(error));
            }
        };

        self.subject_metadata.last_event_hash = last_event_hash;
        self.subject_metadata.sn += 1;

        Ok(())
    }
}

impl Storable for Governance {}
