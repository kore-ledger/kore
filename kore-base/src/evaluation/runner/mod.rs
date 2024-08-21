use std::collections::HashSet;

use actor::{
    Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response,
};
use async_trait::async_trait;
use borsh::{to_vec, BorshDeserialize};
use serde::{Deserialize, Serialize};
use types::{
    Contract, ContractResult, GovernanceData, GovernanceEvent, MemoryManager, WasmContractResult,
};
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};

use crate::{
    governance::{model::SchemaEnum, Member, Policy, Role, Schema, Who},
    model::patch::apply_patch,
    Error, Governance, ValueWrapper,
};

pub mod types;

pub struct Runner {}

impl Runner {
    async fn execute_contract(
        state: &ValueWrapper,
        event: &ValueWrapper,
        compiled_contract: Contract,
        is_owner: bool,
    ) -> Result<ContractResult, Error> {
        let Contract::CompiledContract(contract_bytes) = compiled_contract
        else {
            return Self::execute_governance_contract(state, event).await;
        };
        // TODO: Cambiar esto cuando la parte de compilación esté hecha, el engine no debería ir aquí
        let engine = Engine::new(&Config::default()).unwrap();

        // Module represents a precompiled WebAssembly program that is ready to be instantiated and executed.
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

            Self::get_result(&store, result_ptr)
    }

    async fn execute_governance_contract(
        state: &ValueWrapper,
        event: &ValueWrapper,
    ) -> Result<ContractResult, Error> {
        let Ok(event) =
            serde_json::from_value::<GovernanceEvent>(event.0.clone())
        else {
            return Ok(ContractResult::error());
        };
        match &event {
            GovernanceEvent::Patch { data } => {
                let Ok(patched_state) =
                    apply_patch(data.0.clone(), state.0.clone())
                else {
                    return Ok(ContractResult::error());
                };
                if Self::check_governance_state(&patched_state).is_ok() {
                    Ok(ContractResult {
                        final_state: ValueWrapper(
                            serde_json::to_value(patched_state).unwrap(),
                        ),
                        approval_required: true,
                        success: true,
                    })
                } else {
                    Ok(ContractResult {
                        final_state: state.clone(),
                        approval_required: false,
                        success: false,
                    })
                }
            }
        }
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

        for member in members {
            if !name_set.insert(member.name.clone()) {
                return Err(Error::Runner(
                    "There are duplicate names in members".to_owned(),
                ));
            }
            if !id_set.insert(member.id.clone()) {
                return Err(Error::Runner(
                    "There are duplicate id in members".to_owned(),
                ));
            }
        }
        // TODO: Checkear que el nodo es miembro, no se ha eliminado.

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
        // TODO Checkear que el owner del nodo sigue tiniendo los roles básicos que se han especificado en la meta gov.
        for role in roles {
            if let SchemaEnum::ID { ID } = &role.schema {
                if !policies.contains(ID) {
                    return Err(Error::Runner(format!("The role {} of member {} belongs to an invalid schema.", role.role, role.who)));
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
                }
                _ => {}
            }
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
        let contract_result: WasmContractResult = BorshDeserialize::try_from_slice(bytes)
            .map_err(|e| Error::Runner(format!("Can not generate wasm contract result: {}", e)))?;
        let result = ContractResult {
            final_state: contract_result.final_state,
            approval_required: contract_result.approval_required,
            success: contract_result.success,
        };
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub enum RunnerCommand {
    Run,
}

impl Message for RunnerCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunnerEvent {}

impl Event for RunnerEvent {}

#[derive(Debug, Clone)]
pub enum RunnerResponse {
    Error(Error),
    None,
}

impl Response for RunnerResponse {}

#[async_trait]
impl Actor for Runner {
    type Event = RunnerEvent;
    type Message = RunnerCommand;
    type Response = RunnerResponse;
}

#[async_trait]
impl Handler<Runner> for Runner {
    async fn handle_message(
        &mut self,
        msg: RunnerCommand,
        ctx: &mut ActorContext<Runner>,
    ) -> Result<RunnerResponse, ActorError> {
        Ok(RunnerResponse::None)
    }

    async fn on_event(
        &mut self,
        event: RunnerEvent,
        ctx: &mut ActorContext<Runner>,
    ) {
    }
}
