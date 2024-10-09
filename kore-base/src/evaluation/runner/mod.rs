use std::{collections::HashSet, fmt::format};

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_trait::async_trait;
use borsh::{to_vec, BorshDeserialize};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use types::{
    Contract, ContractResult, GovernanceData, GovernanceEvent, MemoryManager,
    RunnerResult,
};
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};

use crate::{
    governance::{
        model::{Roles, SchemaEnum},
        Member, Policy, Role, Schema, Who,
    },
    model::patch::apply_patch,
    Error, ValueWrapper,
};

pub mod types;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Runner {}

impl Runner {
    async fn execute_contract(
        state: &ValueWrapper,
        event: &ValueWrapper,
        compiled_contract: Contract,
        is_owner: bool,
    ) -> Result<(RunnerResult, Vec<String>), Error> {
        // If it is a governance we do not need to use wastime, since the governance contract is always the same and it is known before starting the node
        let Contract::CompiledContract(contract_bytes) = compiled_contract
        else {
            return Self::execute_governance_contract(state, event).await;
        };
        let engine = Engine::new(&Config::default()).map_err(|e| {
            Error::Runner(format!("Error creating the engine: {}", e))
        })?;

        // Module represents a precompiled WebAssembly program that is ready to be instantiated and executed.
        // This function receives the previous input from Engine::precompile_module, that is why this function can be considered safe.
        let module = unsafe {
            Module::deserialize(&engine, contract_bytes).map_err(|e| {
                Error::Runner(format!(
                    "Error deserializing the contract in wastime: {}",
                    e
                ))
            })?
        };

        // We create a context from the state and the event.
        let (context, state_ptr, event_ptr) =
            Self::generate_context(state, event)?;

        // Container to store and manage the global state of a WebAssembly instance during its execution.
        let mut store = Store::new(&engine, context);

        // Responsible for combining several object files into a single WebAssembly executable file (.wasm).
        let linker = Self::generate_linker(&engine)?;

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
            .get_typed_func::<(u32, u32, u32), u32>(&mut store, "main_function")
            .map_err(|e| {
                Error::Runner(format!("Contract entry point not found: {}", e))
            })?;

        // Contract execution
        let result_ptr = contract_entrypoint
            .call(
                &mut store,
                (state_ptr, event_ptr, if is_owner { 1 } else { 0 }),
            )
            .map_err(|e| {
                Error::Runner(format!("Contract execution failed: {}", e))
            })?;

        let result = Self::get_result(&store, result_ptr)?;
        Ok((
            RunnerResult {
                approval_required: false,
                final_state: result.final_state,
                success: result.success,
            },
            vec![],
        ))
    }

    async fn execute_governance_contract(
        state: &ValueWrapper,
        event: &ValueWrapper,
    ) -> Result<(RunnerResult, Vec<String>), Error> {
        let event = serde_json::from_value::<GovernanceEvent>(event.0.clone())
            .map_err(|e| {
                Error::Runner(format!("Can not create governance event {}", e))
            })?;

        match &event {
            GovernanceEvent::Patch { data } => {
                // TODO estudiar todas las operaciones de jsonpatch, qué pasa si eliminamos un schema del cual hay sujetos?
                // o si hacemos un copy o move. Deberíamos permitirlos?
                let patched_state =
                    apply_patch(data.0.clone(), state.0.clone()).map_err(
                        |e| Error::Runner(format!("Can not apply patch {}", e)),
                    )?;

                if Self::check_governance_state(&patched_state).is_ok() {
                    let compilations = Self::check_compilation(data.0.clone())?;
                    // TODO QUITAR TODOS LOS unwrap()
                    Ok((
                        RunnerResult {
                            final_state: ValueWrapper(
                                serde_json::to_value(patched_state).unwrap(),
                            ),
                            approval_required: true,
                            success: true,
                        },
                        compilations,
                    ))
                } else {
                    todo!()
                }
            }
        }
    }

    fn check_compilation(operations: Value) -> Result<Vec<String>, Error> {
        // En caso de que sea un replace habría que ver realmente si hay algún cambio, solo cambiar el orden es una operación de replace aunque no cambie nada TODO.
        // TODO ver si tenemos que permitir más operaciones de JSON patch y como las abordamos, sobretodo el remove
        let operations_array =
            if let Some(operations_array) = operations.as_array() {
                operations_array
            } else {
                return Err(Error::Runner(
                    "json patch operations are not an array".to_owned(),
                ));
            };

        let mut compilations = vec![];
        for val in operations_array {
            // obtain op
            let op = if let Some(op) = val["op"].as_str() {
                op
            } else {
                return Err(Error::Runner(
                    "json patch operations have no “op” field".to_owned(),
                ));
            };

            // Check if op is add or replace, or remove for roles and members
            match op {
                "add" | "replace" => {},
                "remove" => {
                    // Obtain path
                    let path = if let Some(path) = val["path"].as_str() {
                        path
                    } else {
                        return Err(Error::Runner("The path field is not a str".to_owned()))
                    };
                    if !path.contains("roles") && !path.contains("members") {
                        return Err(Error::Runner(format!("Remove operation in JSON parch is only allowed for members and roles, invalid operation {} for {}", op, path)))   
                    }
                },
                _ => return Err(Error::Runner(format!("The only json patch operations that are allowed are add, replace and remove (only for members and roles), invalid operation: {}", op)))
            }

            // Check if has schema field is a schema
            if !val["value"]["schema"].is_null() {
                let id = if let Some(id) = val["value"]["id"].as_str() {
                    id
                } else {
                    return Err(Error::Runner(
                        "The id field is not a str".to_owned(),
                    ));
                };

                // Save the id that needs to be compiled
                compilations.push(id.to_owned());
            }
        }
        Ok(compilations)
    }

    fn check_governance_state(
        governance: &GovernanceData,
    ) -> Result<(), Error> {
        // Nombre e ID unicos
        let (id_set, name_set) = Self::check_members(&governance.members)?;
        // Políticas únicas
        let policies_names = Self::check_policies(&governance.policies)?;
        // Schemas y politicas 1:1, no puede aparecer el schema de governance
        Self::check_schemas(&governance.schemas, policies_names.clone())?;
        // Los roles tiene que ser asignados a miembros, sea por ID o NAME y tienen que pertenecer a una schema (politicas y schemas son las mismas)
        Self::check_roles(&governance.roles, policies_names, id_set, name_set)
    }

    fn check_members(
        members: &[Member],
    ) -> Result<(HashSet<String>, HashSet<String>), Error> {
        let mut name_set = HashSet::new();
        let mut id_set = HashSet::new();
        let mut owner_name = false;

        for member in members {
            if !name_set.insert(member.name.clone()) {
                return Err(Error::Runner(
                    "There are duplicate names in members".to_owned(),
                ));
            }
            if member.name == "Owner" {
                owner_name = true;
            }
            if !id_set.insert(member.id.clone()) {
                return Err(Error::Runner(
                    "There are duplicate id in members".to_owned(),
                ));
            }
        }

        if !owner_name {
            return Err(Error::Runner(
                "The owner of the governance must be a member of the governance".to_owned(),
            ));
        }

        Ok((id_set, name_set))
    }

    fn check_policies(policies: &[Policy]) -> Result<HashSet<String>, Error> {
        let mut is_governance_present = false;
        let mut id_set = HashSet::new();

        for policy in policies {
            if policy.id != "governance" {
                if !id_set.insert(policy.id.clone()) {
                    return Err(Error::Runner(
                        "There are duplicate id in polities".to_owned(),
                    ));
                }
            } else if !is_governance_present {
                is_governance_present = true;
            } else {
                return Err(Error::Runner(
                    "The policy of governance appears more than once."
                        .to_owned(),
                ));
            }
        }

        if !is_governance_present {
            return Err(Error::Runner(
                "governance policy is not present".to_owned(),
            ));
        }

        Ok(id_set)
    }

    fn check_schemas(
        schemas: &[Schema],
        mut policies_names: HashSet<String>,
    ) -> Result<(), Error> {
        // También se tiene que comprobar que los estados iniciales sean válidos según el json_schema, TODO
        for schema in schemas {
            if &schema.id == "governance" {
                return Err(Error::Runner(
                    "There cannot exist a schema with the name governance"
                        .to_owned(),
                ));
            }

            if !policies_names.remove(&schema.id) {
                // Error hay un schema que no está en policies
                return Err(Error::Runner(
                    "There is a schema that has no associated policy"
                        .to_owned(),
                ));
            }
        }

        if !policies_names.is_empty() {
            // Error hay un policie que no está en schema.
            return Err(Error::Runner(
                "There is a policy that has no associated schema".to_owned(),
            ));
        }
        Ok(())
    }

    fn check_roles(
        roles: &[Role],
        mut policies: HashSet<String>,
        id_set: HashSet<String>,
        name_set: HashSet<String>,
    ) -> Result<(), Error> {
        policies.insert("governance".into());
        let mut owner_eval = false;
        let mut owner_appr = false;
        let mut owner_val = false;
        let mut owner_witness = false;
        let mut members_witness = false;

        for role in roles {
            if let Who::NOT_MEMBERS = role.who {
                if role.role != Roles::ISSUER {
                    return Err(Error::Runner("The user NOT_MEMBERS only can be assigned to ISSUER rol".to_owned()));
                }
            };

            if let Roles::APPROVER = role.role {
                if role.schema == SchemaEnum::ALL
                    || SchemaEnum::NOT_GOVERNANCE == role.schema
                {
                    return Err(Error::Runner("The approver role only can be asing to governance schema".to_owned()));
                }
            };

            if let SchemaEnum::ID { ID } = &role.schema {
                if !policies.contains(ID) {
                    return Err(Error::Runner(format!("The role {} of member {} belongs to an invalid schema.", role.role, role.who)));
                }
                if ID == "governance" {
                    if let Who::MEMBERS = role.who {
                        if role.role == Roles::WITNESS
                            && role.namespace.is_empty()
                        {
                            members_witness = true;
                        }
                    }
                }
            }
            match &role.who {
                Who::ID { ID } => {
                    if !id_set.contains(ID) {
                        return Err(Error::Runner(format!(
                            "No members have been added with this ID: {}",
                            ID
                        )));
                    }
                }
                Who::NAME { NAME } => {
                    if !name_set.contains(NAME) {
                        return Err(Error::Runner(format!(
                            "No members have been added with this ID: {}",
                            NAME
                        )));
                    }
                    if NAME == "Owner" && role.namespace.is_empty() {
                        if let SchemaEnum::ALL = role.schema {
                            match role.role {
                                Roles::APPROVER => owner_appr = true,
                                Roles::EVALUATOR => owner_eval = true,
                                Roles::VALIDATOR => owner_val = true,
                                Roles::WITNESS => owner_witness = true,
                                _ => {}
                            };
                        }
                    }
                }
                _ => {}
            }
        }
        if !owner_eval
            || !owner_appr
            || !owner_val
            || !owner_witness
            || !members_witness
        {
            return Err(Error::Runner(
                "Basic metagovernance roles have been modified".to_owned(),
            ));
        }

        Ok(())
    }

    fn generate_linker(
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
                Error::Runner(format!("An error has occurred linking a function, module: env, name: pointer_len, {}", e))
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
                Error::Runner(format!("An error has occurred linking a function, module: env, name: allow, {}", e))
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
                Error::Runner(format!("An error has occurred linking a function, module: env, name: write_byte, {}", e))
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
                Error::Runner(format!("An error has occurred linking a function, module: env, name: read_byte, {}", e))
            })?;

        linker
            .func_wrap(
                "env",
                "cout",
                |_caller: Caller<'_, MemoryManager>, ptr: u32| {
                    println!("{}", ptr);
                },
            )
            .map_err(|e| {
                Error::Runner(format!("An error has occurred linking a function, module: env, name: cout, {}", e))
            })?;
        Ok(linker)
    }

    fn generate_context(
        state: &ValueWrapper,
        event: &ValueWrapper,
    ) -> Result<(MemoryManager, u32, u32), Error> {
        let mut context = MemoryManager::new();
        let state_bytes = to_vec(&state).map_err(|e| {
            Error::Runner(format!(
                "Error when serializing the state using borsh: {}",
                e
            ))
        })?;
        let state_ptr = context.add_data_raw(&state_bytes);
        let event_bytes = to_vec(&event).map_err(|e| {
            Error::Runner(format!(
                "Error when serializing the event using borsh: {}",
                e
            ))
        })?;
        let event_ptr = context.add_data_raw(&event_bytes);
        Ok((context, state_ptr as u32, event_ptr as u32))
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
            todo!()
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunnerMessage {
    pub state: ValueWrapper,
    pub event: ValueWrapper,
    pub compiled_contract: Contract,
    pub is_owner: bool,
}

impl Message for RunnerMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunnerEvent {}

impl Event for RunnerEvent {}

#[derive(Debug, Clone)]
pub enum RunnerResponse {
    Response {
        result: RunnerResult,
        compilations: Vec<String>,
    },
    Error(Error),
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
        sender: ActorPath,
        msg: RunnerMessage,
        _ctx: &mut ActorContext<Runner>,
    ) -> Result<RunnerResponse, ActorError> {
        match Self::execute_contract(
            &msg.state,
            &msg.event,
            msg.compiled_contract,
            msg.is_owner,
        )
        .await
        {
            Ok((result, compilations)) => Ok(RunnerResponse::Response {
                result,
                compilations,
            }),
            Err(e) => Ok(RunnerResponse::Error(e)),
        }
    }
}
