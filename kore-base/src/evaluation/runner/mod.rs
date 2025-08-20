// Copyright 2025 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_trait::async_trait;
use borsh::{BorshDeserialize, to_vec};
use identity::identifier::KeyIdentifier;
use json_patch::diff;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use tracing::error;
use types::{ContractResult, EvaluateType, RunnerResult};
use wasmtime::{Config, Engine, Module, Store};

use crate::{
    Error, ValueWrapper,
    governance::{
        Governance, Schema,
        events::{
            GovernanceEvent, MemberEvent, PoliciesEvent, RolesEvent,
            SchemasEvent,
        },
        model::{HashThisRole, RoleTypes},
    },
    model::{
        Namespace,
        common::{MemoryManager, generate_linker},
        patch::apply_patch,
    },
};

const TARGET_RUNNER: &str = "Kore-Evaluation-Runner";
type ChangeRemoveMembers = (Vec<(String, String)>, Vec<String>);
type AddRemoveChangeSchema =
    (HashSet<String>, HashSet<String>, HashSet<String>);

pub mod types;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Runner {}

impl Runner {
    async fn execute_contract(
        state: &ValueWrapper,
        evaluate_type: EvaluateType,
        is_owner: bool,
    ) -> Result<(RunnerResult, Vec<String>), Error> {
        match evaluate_type {
            EvaluateType::AllSchemasFact {
                contract,
                init_state,
                payload,
            } => {
                Self::execute_fact_not_gov(
                    state,
                    &init_state,
                    &payload,
                    &contract,
                    is_owner,
                )
                .await
            }
            EvaluateType::GovFact { payload } => {
                Self::execute_fact_gov(state, &payload).await
            }
            EvaluateType::GovTransfer { new_owner } => {
                Self::execute_transfer_gov(state.clone(), &new_owner)
            }
            EvaluateType::AllSchemasTransfer {
                new_owner,
                old_owner,
                namespace,
                schema_id,
            } => Self::execute_transfer_not_gov(
                state.clone(),
                &new_owner,
                &old_owner,
                namespace,
                &schema_id,
            ),
            EvaluateType::GovConfirm {
                old_owner_name,
                new_owner,
            } => Self::execute_confirm_gov(state, old_owner_name, &new_owner),
        }
    }

    fn execute_transfer_not_gov(
        state: ValueWrapper,
        new_owner: &KeyIdentifier,
        old_owner: &KeyIdentifier,
        namespace: Namespace,
        schema_id: &str,
    ) -> Result<(RunnerResult, Vec<String>), Error> {
        if new_owner.is_empty() {
            return Err(Error::Runner(
                "New owner KeyIdentifier can not be empty".to_owned(),
            ));
        }

        if new_owner == old_owner {
            return Err(Error::Runner(
                "The new owner is now the owner".to_owned(),
            ));
        }

        let governance = serde_json::from_value::<Governance>(state.0)
            .map_err(|e| {
                Error::Runner(format!("Can deserialice governance patch {}", e))
            })?;

        if !governance.is_member(new_owner) {
            return Err(Error::Runner(
                "New owner is not a member of governance".to_owned(),
            ));
        }

        if !governance.has_this_role(HashThisRole::Schema {
            who: new_owner.clone(),
            role: RoleTypes::Creator,
            schema_id: schema_id.to_owned(),
            namespace: namespace.clone(),
        }) {
            return Err(Error::Runner(format!(
                "New owner is not a Creator from {} schema_id, with {} namespace",
                schema_id, namespace
            )));
        }

        Ok((
            RunnerResult {
                approval_required: false,
                final_state: ValueWrapper(serde_json::Value::String(
                    "[]".to_owned(),
                )),
            },
            vec![],
        ))
    }

    fn execute_transfer_gov(
        state: ValueWrapper,
        new_owner: &KeyIdentifier,
    ) -> Result<(RunnerResult, Vec<String>), Error> {
        if new_owner.is_empty() {
            return Err(Error::Runner(
                "New owner KeyIdentifier can not be empty".to_owned(),
            ));
        }

        let governance = serde_json::from_value::<Governance>(state.0)
            .map_err(|e| {
                Error::Runner(format!("Can deserialice governance patch {}", e))
            })?;

        let Some(owner_key) = governance.members.get("Owner") else {
            return Err(Error::Runner(
                "Can not get Owner into members".to_owned(),
            ));
        };

        if owner_key == new_owner {
            return Err(Error::Runner(
                "The new owner is now the owner".to_owned(),
            ));
        }

        if !governance.is_member(new_owner) {
            return Err(Error::Runner(
                "New owner is not a member of governance".to_owned(),
            ));
        }

        Ok((
            RunnerResult {
                approval_required: false,
                final_state: ValueWrapper(serde_json::Value::String(
                    "[]".to_owned(),
                )),
            },
            vec![],
        ))
    }

    fn execute_confirm_gov(
        state: &ValueWrapper,
        old_owner_name: Option<String>,
        new_owner: &KeyIdentifier,
    ) -> Result<(RunnerResult, Vec<String>), Error> {
        if new_owner.is_empty() {
            return Err(Error::Runner(
                "New owner KeyIdentifier can not be empty".to_owned(),
            ));
        }

        let mut governance = serde_json::from_value::<Governance>(
            state.0.clone(),
        )
        .map_err(|e| {
            Error::Runner(format!("Can deserialice governance patch {}", e))
        })?;

        let Some(old_owner_key) = governance.members.get("Owner").cloned()
        else {
            return Err(Error::Runner(
                "Cannot find 'Owner' member in governance".to_owned(),
            ));
        };

        let Some(new_owner_member) = governance
            .members
            .iter()
            .find(|x| x.1 == new_owner)
            .map(|x| x.0)
            .cloned()
        else {
            return Err(Error::Runner(
                "Cannot find new_owner member in governance".to_owned(),
            ));
        };

        governance
            .members
            .insert("Owner".to_owned(), new_owner.clone());
        governance.members.remove(&new_owner_member);

        governance
            .change_name_role(&vec![(new_owner_member, "Owner".to_owned())]);

        if let Some(mut old_owner_name) = old_owner_name {
            old_owner_name = old_owner_name.trim().to_owned();

            if old_owner_name.is_empty() {
                return Err(Error::Runner(
                    "New name to old owner can not be empty".to_owned(),
                ));
            }

            if old_owner_name.len() > 50 {
                return Err(Error::Runner(format!(
                    "The size of the new name of the old owner must be less than or equal to 50 characters: {}",
                    old_owner_name
                )));
            }

            if governance
                .members
                .insert(old_owner_name.clone(), old_owner_key.clone())
                .is_some()
            {
                return Err(Error::Runner(format!(
                    "Can not add old owner as member: '{}' already exists",
                    old_owner_name
                )));
            }

            governance.roles_gov.witness.insert(old_owner_name);
        }

        let mod_state = to_value(governance).map_err(|e| {
            Error::Runner(format!(
                "Can not serialice Governance into Value {}",
                e
            ))
        })?;

        let patch = diff(&state.0, &mod_state);
        let json_patch = to_value(patch).map_err(|e| {
            Error::Runner(format!("Can not conver patch to JSON patch: {}", e))
        })?;
        let patched_state: Governance =
            apply_patch(json_patch.clone(), state.0.clone()).map_err(|e| {
                Error::Runner(format!("Can not apply patch {}", e))
            })?;
        Ok((
            RunnerResult {
                final_state: ValueWrapper(to_value(patched_state).map_err(
                    |e| {
                        Error::Runner(format!(
                            "Can not conver patched state into Value: {}",
                            e
                        ))
                    },
                )?),
                approval_required: false,
            },
            vec![],
        ))
    }

    async fn execute_fact_not_gov(
        state: &ValueWrapper,
        init_state: &ValueWrapper,
        payload: &ValueWrapper,
        contract: &[u8],
        is_owner: bool,
    ) -> Result<(RunnerResult, Vec<String>), Error> {
        let engine = Engine::new(&Config::default()).map_err(|e| {
            Error::Runner(format!("Error creating the engine: {}", e))
        })?;

        // Module represents a precompiled WebAssembly program that is ready to be instantiated and executed.
        // This function receives the previous input from Engine::precompile_module, that is why this function can be considered safe.
        let module = unsafe {
            Module::deserialize(&engine, contract).map_err(|e| {
                Error::Runner(format!(
                    "Error deserializing the contract in wastime: {}",
                    e
                ))
            })?
        };

        // We create a context from the state and the event.
        let (context, state_ptr, init_state_ptr, event_ptr) =
            Self::generate_context(state, init_state, payload)?;

        // Container to store and manage the global state of a WebAssembly instance during its execution.
        let mut store = Store::new(&engine, context);

        // Responsible for combining several object files into a single WebAssembly executable file (.wasm).
        let linker = generate_linker(&engine)?;

        // Contract instance.
        let instance =
            linker.instantiate(&mut store, &module).map_err(|e| {
                Error::Runner(format!(
                    "Error when creating a contract instance: {}",
                    e
                ))
            })?;

        // Get access to contract
        let contract_entrypoint = instance
            .get_typed_func::<(u32, u32, u32, u32), u32>(
                &mut store,
                "main_function",
            )
            .map_err(|e| {
                Error::Runner(format!("Contract entry point not found: {}", e))
            })?;

        // Contract execution
        let result_ptr = contract_entrypoint
            .call(
                &mut store,
                (
                    state_ptr,
                    init_state_ptr,
                    event_ptr,
                    if is_owner { 1 } else { 0 },
                ),
            )
            .map_err(|e| {
                Error::Runner(format!("Contract execution failed: {}", e))
            })?;

        let result = Self::get_result(&store, result_ptr)?;
        if !result.success {
            return Err(Error::Runner("Contract was not succes".to_owned()));
        }

        Ok((
            RunnerResult {
                approval_required: false,
                final_state: result.final_state,
            },
            vec![],
        ))
    }

    async fn execute_fact_gov(
        state: &ValueWrapper,
        event: &ValueWrapper,
    ) -> Result<(RunnerResult, Vec<String>), Error> {
        let mut governance: Governance =
            serde_json::from_value(state.0.clone()).map_err(|e| {
                Error::Runner(format!(
                    "Can not deserialize Value into Governance: {}",
                    e
                ))
            })?;
        let event: GovernanceEvent = serde_json::from_value(event.0.clone())
            .map_err(|e| {
                Error::Runner(format!(
                    "Can not deserialize events into GovernanceEvent: {}",
                    e
                ))
            })?;

        if event.is_empty() {
            return Err(Error::Runner("Event can not be empty".to_owned()));
        }

        if let Some(member_event) = event.members {
            let (change_name, remove) =
                Self::check_members(&member_event, &mut governance)?;
            if !remove.is_empty() {
                governance.remove_member_role(&remove);
            }

            if !change_name.is_empty() {
                governance.change_name_role(&change_name);
            }
        }

        let add_change_schemas = if let Some(schema_event) = event.schemas {
            let (add_schemas, remove_schemas, change_schemas) =
                Self::check_schemas(&schema_event, &mut governance)?;
            governance.remove_schema(remove_schemas);
            governance.add_schema(add_schemas.clone());

            add_schemas
                .union(&change_schemas)
                .cloned()
                .collect::<Vec<String>>()
        } else {
            vec![]
        };

        if let Some(roles_event) = event.roles {
            Self::check_roles(roles_event, &mut governance)?;
        }

        if let Some(policies_event) = event.policies {
            Self::check_policies(policies_event, &mut governance)?;
        }

        if !governance.check_basic_gov() {
            return Err(Error::Runner(
                "The basic roles of the governance owner have been modified"
                    .to_owned(),
            ));
        }

        let mod_state = to_value(governance).map_err(|e| {
            Error::Runner(format!(
                "Can not serialice Governance into Value {}",
                e
            ))
        })?;

        let patch = diff(&state.0, &mod_state);
        let json_patch = to_value(patch).map_err(|e| {
            Error::Runner(format!("Can not conver patch to JSON patch: {}", e))
        })?;

        let patched_state: Governance =
            apply_patch(json_patch.clone(), state.0.clone()).map_err(|e| {
                Error::Runner(format!("Can not apply patch {}", e))
            })?;

        Ok((
            RunnerResult {
                final_state: ValueWrapper(to_value(patched_state).map_err(
                    |e| {
                        Error::Runner(format!(
                            "Can not conver patched state into Value: {}",
                            e
                        ))
                    },
                )?),
                approval_required: true,
            },
            add_change_schemas,
        ))
    }

    fn check_policies(
        policies_event: PoliciesEvent,
        governance: &mut Governance,
    ) -> Result<(), Error> {
        if policies_event.is_empty() {
            return Err(Error::Runner(
                "PoliciesEvent can not be empty".to_owned(),
            ));
        }

        if let Some(gov) = policies_event.governance {
            if gov.change.is_empty() {
                return Err(Error::Runner(
                    "Change in GovPolicieEvent can not be empty".to_owned(),
                ));
            }

            let mut new_policies = governance.policies_gov.clone();

            if let Some(approve) = gov.change.approve {
                if let Err(e) = approve.check_values() {
                    return Err(Error::Runner(format!(
                        "Gov change policies, approve quorum error: {}",
                        e
                    )));
                }

                new_policies.approve = approve;
            }

            if let Some(evaluate) = gov.change.evaluate {
                if let Err(e) = evaluate.check_values() {
                    return Err(Error::Runner(format!(
                        "Gov change policies, evaluate quorum error: {}",
                        e
                    )));
                }

                new_policies.evaluate = evaluate;
            }

            if let Some(validate) = gov.change.validate {
                if let Err(e) = validate.check_values() {
                    return Err(Error::Runner(format!(
                        "Gov change policies, validate quorum error: {}",
                        e
                    )));
                }

                new_policies.validate = validate;
            }

            governance.policies_gov = new_policies;
        }

        if let Some(schemas) = policies_event.schema {
            if schemas.is_empty() {
                return Err(Error::Runner(
                    "SchemaIdPolicie vec can not be empty".to_owned(),
                ));
            }

            let mut new_policies = governance.policies_schema.clone();

            for schema in schemas {
                if schema.is_empty() {
                    return Err(Error::Runner(
                        "SchemaIdPolicie can not be empty".to_owned(),
                    ));
                }

                let Some(policies_schema) =
                    new_policies.get_mut(&schema.schema_id)
                else {
                    return Err(Error::Runner(format!(
                        "{} is not a schema",
                        schema.schema_id
                    )));
                };

                if let Some(change) = schema.policies.change {
                    if change.is_empty() {
                        return Err(Error::Runner(
                            "Change in SchemaIdPolicie can not be empty"
                                .to_owned(),
                        ));
                    }

                    if let Some(evaluate) = change.evaluate {
                        if let Err(e) = evaluate.check_values() {
                            return Err(Error::Runner(format!(
                                "Schema {} change policies, evaluate quorum error: {}",
                                schema.schema_id, e
                            )));
                        }

                        policies_schema.evaluate = evaluate;
                    }

                    if let Some(validate) = change.validate {
                        if let Err(e) = validate.check_values() {
                            return Err(Error::Runner(format!(
                                "Schema {} change policies, validate quorum error: {}",
                                schema.schema_id, e
                            )));
                        }

                        policies_schema.validate = validate;
                    }
                }
            }

            governance.policies_schema = new_policies
        }

        Ok(())
    }

    fn check_roles(
        roles_event: RolesEvent,
        governance: &mut Governance,
    ) -> Result<(), Error> {
        if roles_event.is_empty() {
            return Err(Error::Runner(
                "RolesEvent can not be empty".to_owned(),
            ));
        }

        if let Some(gov) = roles_event.governance {
            if gov.is_empty() {
                return Err(Error::Runner(
                    "GovRoleEvent can not be empty".to_owned(),
                ));
            }

            let mut new_roles = governance.roles_gov.clone();

            gov.check_data(governance, &mut new_roles)?;

            governance.roles_gov = new_roles;
        }

        if let Some(schemas) = roles_event.schema {
            if schemas.is_empty() {
                return Err(Error::Runner(
                    "SchemaIdRole vec can not be empty".to_owned(),
                ));
            }

            let mut new_roles = governance.roles_schema.clone();

            for schema in schemas {
                if schema.is_empty() {
                    return Err(Error::Runner(
                        "SchemaIdRole can not be empty".to_owned(),
                    ));
                }

                let Some(roles_schema) = new_roles.get_mut(&schema.schema_id)
                else {
                    return Err(Error::Runner(format!(
                        "{} is not a schema",
                        schema.schema_id
                    )));
                };

                schema.roles.check_data(
                    governance,
                    roles_schema,
                    &schema.schema_id,
                )?
            }

            governance.roles_schema = new_roles;
        }

        if let Some(all_schemas) = roles_event.all_schemas {
            let new_roles = governance.roles_all_schemas.clone();

            let new_roles =
                all_schemas.check_data(governance, new_roles, "all_schemas")?;

            governance.roles_all_schemas = new_roles;
        }

        Ok(())
    }

    fn check_schemas(
        schema_event: &SchemasEvent,
        governance: &mut Governance,
    ) -> Result<AddRemoveChangeSchema, Error> {
        if schema_event.is_empty() {
            return Err(Error::Runner(
                "SchemasEvent can not be empty".to_owned(),
            ));
        }

        let mut remove_schemas = HashSet::new();
        let mut add_schemas = HashSet::new();
        let mut change_schemas = HashSet::new();

        let mut new_schemas = governance.schemas.clone();

        if let Some(add) = schema_event.add.clone() {
            if add.is_empty() {
                return Err(Error::Runner(
                    "SchemaAdd vec can not be empty".to_owned(),
                ));
            }

            for mut new_schema in add {
                new_schema.id = new_schema.id.trim().to_owned();

                if new_schema.id.is_empty() {
                    return Err(Error::Runner(
                        "Id of schema to add can not be empty".to_owned(),
                    ));
                }

                if new_schema.id.len() > 50 {
                    return Err(Error::Runner(format!(
                        "The size of the schema ID must be less than or equal to 50 characters: {}",
                        new_schema.id
                    )));
                }

                if new_schema.id == "all_schemas" {
                    return Err(Error::Runner("There can not be a schema whose id is all_schemas, it is a reserved word".to_owned()));
                }

                if new_schema.id == "governance" {
                    return Err(Error::Runner("There can not be a schema whose id is governance, it is a reserved word".to_owned()));
                }

                if new_schema.contract.is_empty() {
                    return Err(Error::Runner(format!(
                        "Contract of schema to add can not be empty, in {}",
                        new_schema.id
                    )));
                }

                if new_schema.contract.contains(" ") {
                    return Err(Error::Runner(format!(
                        "Contract of schema to add can not contains blank spaces, in {}",
                        new_schema.id
                    )));
                }

                if new_schemas
                    .insert(
                        new_schema.id.clone(),
                        Schema {
                            initial_value: new_schema.initial_value,
                            contract: new_schema.contract,
                        },
                    )
                    .is_some()
                {
                    return Err(Error::Runner(format!(
                        "There is already a schema with this id: {}",
                        new_schema.id
                    )));
                };

                add_schemas.insert(new_schema.id);
            }
        }

        if let Some(remove) = schema_event.remove.clone() {
            if remove.is_empty() {
                return Err(Error::Runner(
                    "String vec can not be empty in remove schema".to_owned(),
                ));
            }

            for remove_schema in remove.clone() {
                if remove_schema.is_empty() {
                    return Err(Error::Runner(
                        "Id of schema to remove can not be empty".to_owned(),
                    ));
                }

                if new_schemas.remove(&remove_schema).is_none() {
                    return Err(Error::Runner(format!(
                        "Can not remove {}, is not a schema",
                        remove_schema
                    )));
                }
            }

            remove_schemas = remove;
        }

        if let Some(change) = schema_event.change.clone() {
            if change.is_empty() {
                return Err(Error::Runner(
                    "SchemaChange vec can not be empty in change schema"
                        .to_owned(),
                ));
            }

            for change_schema in change {
                if change_schema.is_empty() {
                    return Err(Error::Runner(format!(
                        "Can not change a schema {}, has empty data",
                        change_schema.actual_id
                    )));
                }

                let Some(schema_data) =
                    new_schemas.get_mut(&change_schema.actual_id)
                else {
                    return Err(Error::Runner(format!(
                        "Can not change a schema, {} is not a actual schema",
                        change_schema.actual_id
                    )));
                };

                if let Some(new_contract) = change_schema.new_contract {
                    if new_contract.is_empty() {
                        return Err(Error::Runner(format!(
                            "Contract of schema to add can not be empty, in {}",
                            change_schema.actual_id
                        )));
                    }

                    if new_contract.contains(" ") {
                        return Err(Error::Runner(format!(
                            "Contract of schema to change can not contains blank spaces, in {}",
                            change_schema.actual_id
                        )));
                    }

                    schema_data.contract = new_contract;
                }

                if let Some(init_value) = change_schema.new_initial_value {
                    schema_data.initial_value = init_value;
                }

                change_schemas.insert(change_schema.actual_id);
            }
        }

        governance.schemas = new_schemas;
        Ok((add_schemas, remove_schemas, change_schemas))
    }

    fn check_members(
        member_event: &MemberEvent,
        governance: &mut Governance,
    ) -> Result<ChangeRemoveMembers, Error> {
        if member_event.is_empty() {
            return Err(Error::Runner(
                "MemberEvent can not be empty".to_owned(),
            ));
        }

        let mut new_members = governance.members.clone();
        if let Some(add) = member_event.add.clone() {
            if add.is_empty() {
                return Err(Error::Runner(
                    "NewMember vec can not be empty in add member".to_owned(),
                ));
            }

            for mut new_member in add {
                new_member.name = new_member.name.trim().to_owned();

                if new_member.name.is_empty() {
                    return Err(Error::Runner(
                        "Name of member to add can not be empty".to_owned(),
                    ));
                }

                if new_member.name.len() > 50 {
                    return Err(Error::Runner(format!(
                        "The size of the member name be less than or equal to 50 characters: {}",
                        new_member.name
                    )));
                }

                if new_member.name == "Any" {
                    return Err(Error::Runner(format!(
                        "Name of member to add can not be 'Any', {}",
                        new_member.name
                    )));
                }

                if new_member.name == "Witnesses" {
                    return Err(Error::Runner(format!(
                        "Name of member to add can not be 'Witnesses', {}",
                        new_member.name
                    )));
                }

                if new_member.key.is_empty() {
                    return Err(Error::Runner(format!(
                        "Key of {} can not be empty",
                        new_member.name
                    )));
                }

                if new_members
                    .insert(new_member.name.clone(), new_member.key)
                    .is_some()
                {
                    return Err(Error::Runner(format!(
                        "There is already a member with this name: {}",
                        new_member.name
                    )));
                };
            }
        }

        let mut remove_members = vec![];
        if let Some(remove) = member_event.remove.clone() {
            if remove.is_empty() {
                return Err(Error::Runner(
                    "String vec can not be empty in remove member".to_owned(),
                ));
            }
            for remove_member in remove.clone() {
                if remove_member == "Owner" {
                    return Err(Error::Runner(
                        "Can not remove Owner of governance".to_owned(),
                    ));
                }

                if remove_member.is_empty() {
                    return Err(Error::Runner(
                        "Name of member to remove can not be empty".to_owned(),
                    ));
                }

                if new_members.remove(&remove_member).is_none() {
                    return Err(Error::Runner(format!(
                        "Can not remove {}, is not a member",
                        remove_member
                    )));
                }
            }

            remove_members = remove.iter().cloned().collect::<Vec<String>>();
        }

        let mut change_name_members: Vec<(String, String)> = vec![];
        if let Some(change) = member_event.change.clone() {
            if change.is_empty() {
                return Err(Error::Runner(
                    "ChangeMember vec can not be empty in change member"
                        .to_owned(),
                ));
            }

            for change_member in change {
                if change_member.actual_name == "Owner" {
                    return Err(Error::Runner(
                        "Can not change Owner of governance".to_owned(),
                    ));
                }

                if change_member.is_empty() {
                    return Err(Error::Runner(format!(
                        "Can not change a member {}, has empty data",
                        change_member.actual_name
                    )));
                }

                let mut actual_member_name = change_member.actual_name.clone();
                let Some(mut actual_member_key) =
                    new_members.remove(&change_member.actual_name)
                else {
                    return Err(Error::Runner(format!(
                        "Can not change a member, {} is not a actual member",
                        change_member.actual_name
                    )));
                };

                if let Some(mut new_name) = change_member.new_name.clone() {
                    new_name = new_name.trim().to_owned();

                    if new_name.is_empty() {
                        return Err(Error::Runner(format!(
                            "New name of {} can not be empty",
                            actual_member_name
                        )));
                    }

                    if new_name.len() > 50 {
                        return Err(Error::Runner(format!(
                            "The size of the new name must be less than or equal to 50 characters: {}",
                            actual_member_name
                        )));
                    }

                    if new_name == "Any" {
                        return Err(Error::Runner(format!(
                            "New name of member can not be 'Any', {}",
                            new_name
                        )));
                    }

                    if new_name == "Witnesses" {
                        return Err(Error::Runner(format!(
                            "New name of member can not be 'Witnesses', {}",
                            new_name
                        )));
                    }

                    change_name_members
                        .push((actual_member_name, new_name.clone()));
                    actual_member_name = new_name;
                };

                if let Some(new_key) = change_member.new_key.clone() {
                    if new_key.is_empty() {
                        return Err(Error::Runner(format!(
                            "New key of {} can not be empty",
                            actual_member_name
                        )));
                    }

                    actual_member_key = new_key;
                };

                if new_members
                    .insert(actual_member_name.clone(), actual_member_key)
                    .is_some()
                {
                    return Err(Error::Runner(format!(
                        "There is already a member with this name: {}",
                        actual_member_name
                    )));
                };
            }
        }

        let members_name: HashSet<String> =
            new_members.keys().cloned().collect();
        let members_value: HashSet<KeyIdentifier> =
            new_members.values().cloned().collect();

        if new_members.contains_key("Any") {
            return Err(Error::Runner("There can not be a member whose name is Any, it is a reserved word".to_owned()));
        }

        if new_members.contains_key("Witnesses") {
            return Err(Error::Runner("There can not be a member whose name is Witnesses, it is a reserved word".to_owned()));
        }

        if members_name.len() != members_value.len() {
            return Err(Error::Runner(
                "There are members who have the same key".to_owned(),
            ));
        }

        governance.members = new_members;

        Ok((change_name_members, remove_members))
    }

    fn generate_context(
        state: &ValueWrapper,
        init_state: &ValueWrapper,
        event: &ValueWrapper,
    ) -> Result<(MemoryManager, u32, u32, u32), Error> {
        let mut context = MemoryManager::default();

        let state_bytes = to_vec(&state).map_err(|e| {
            Error::Runner(format!(
                "Error when serializing the state using borsh: {}",
                e
            ))
        })?;
        let state_ptr = context.add_data_raw(&state_bytes);

        let init_state_bytes = to_vec(&init_state).map_err(|e| {
            Error::Runner(format!(
                "Error when serializing the init_state using borsh: {}",
                e
            ))
        })?;
        let init_state_ptr = context.add_data_raw(&init_state_bytes);

        let event_bytes = to_vec(&event).map_err(|e| {
            Error::Runner(format!(
                "Error when serializing the event using borsh: {}",
                e
            ))
        })?;
        let event_ptr = context.add_data_raw(&event_bytes);
        Ok((
            context,
            state_ptr as u32,
            init_state_ptr as u32,
            event_ptr as u32,
        ))
    }

    fn get_result(
        store: &Store<MemoryManager>,
        pointer: u32,
    ) -> Result<ContractResult, Error> {
        let bytes = store.data().read_data(pointer as usize)?;
        let contract_result: ContractResult =
            BorshDeserialize::try_from_slice(bytes).map_err(|e| {
                Error::Runner(format!(
                    "Can not generate wasm contract result: {}",
                    e
                ))
            })?;

        if contract_result.success {
            Ok(contract_result)
        } else {
            Err(Error::Runner(format!(
                "Contract execution in running was not successful: {}",
                contract_result.error
            )))
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunnerMessage {
    pub state: ValueWrapper,
    pub evaluate_type: EvaluateType,
    pub is_owner: bool,
}

impl Message for RunnerMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunnerEvent {}

impl Event for RunnerEvent {}

#[derive(Debug, Clone)]
pub struct RunnerResponse {
    pub result: RunnerResult,
    pub compilations: Vec<String>,
}

impl Response for RunnerResponse {}

#[async_trait]
impl Actor for Runner {
    type Event = RunnerEvent;
    type Message = RunnerMessage;
    type Response = RunnerResponse;
}

#[async_trait]
impl Handler<Runner> for Runner {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: RunnerMessage,
        _ctx: &mut ActorContext<Runner>,
    ) -> Result<RunnerResponse, ActorError> {
        let (result, compilations) =
            Self::execute_contract(&msg.state, msg.evaluate_type, msg.is_owner)
                .await
                .map_err(|e| {
                    error!(TARGET_RUNNER, "A problem running contract: {}", e);
                    ActorError::Functional(e.to_string())
                })?;

        Ok(RunnerResponse {
            result,
            compilations,
        })
    }
}
