use std::{collections::HashSet, process::Command};

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_std::fs;
use async_trait::async_trait;
use borsh::{to_vec, BorshDeserialize, BorshSerialize};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::error;
use wasmtime::{Caller, Config, Engine, ExternType, Linker, Module, Store};

use crate::{
    governance::json_schema::JsonSchema, Error, HashId, ValueWrapper,
    CONTRACTS, DIGEST_DERIVATOR,
};

use super::runner::types::MemoryManager;

#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone,
)]
pub struct ContractResult {
    pub success: bool,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Compiler {
    pub contract: DigestIdentifier,
}

impl Compiler {
    async fn compile_contract(
        contract: &str,
        contract_path: &str,
    ) -> Result<(), Error> {
        // Write contract.
        fs::write(format!("contracts/src/bin/{}.rs", contract_path), contract)
            .await
            .map_err(|e| {
                Error::Compiler(format!(
                    "Can not create contracts/src/{} file: {}",
                    contract_path, e
                ))
            })?;

        // Compiling contract
        let status = Command::new("cargo")
            .arg("build")
            .arg("--manifest-path=contracts/Cargo.toml")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .arg("--release")
            .arg("--bin")
            .arg(contract_path)
            .output()
            // Does not show stdout. Generates child process and waits
            .map_err(|e| {
                Error::Compiler(format!(
                    "Can not compile contract {}: {}",
                    contract_path, e
                ))
            })?;

        // Is success
        if !status.status.success() {
            return Err(Error::Compiler(format!(
                "Can not compile {}",
                contract_path
            )));
        }

        Ok(())
    }

    async fn check_wasm(contract_path: &str, state: ValueWrapper) -> Result<Vec<u8>, Error> {
        // Read compile contract
        let file = fs::read(format!(
            "contracts/target/wasm32-unknown-unknown/release/{}.wasm",
            contract_path
        ))
        .await
        .map_err(|e| {
            Error::Compiler(format!(
                "Can not read compile contract {}: {}",
                contract_path, e
            ))
        })?;

        let engine = Engine::new(&Config::default()).map_err(|e| {
            Error::Compiler(format!("Error creating the engine: {}", e))
        })?;

        // Precompilation
        let contract_bytes = engine.precompile_module(&file).map_err(|e| {
            Error::Compiler(format!(
                "Can not precompile contract {} with wasmtime engine: {}",
                contract_path, e
            ))
        })?;

        // Module represents a precompiled WebAssembly program that is ready to be instantiated and executed.
        // This function receives the previous input from Engine::precompile_module, that is why this function can be considered safe.
        let module = unsafe {
            Module::deserialize(&engine, contract_bytes.clone()).map_err(
                |e| {
                    Error::Runner(format!(
                        "Error deserializing the contract in wastime: {}",
                        e
                    ))
                },
            )?
        };

        // Obtain imports
        let imports = module.imports();
        // get functions of sdk
        let mut pending_sdk = Self::get_sdk_functions_identifier();

        for import in imports {
            // import must be a function
            match import.ty() {
                ExternType::Func(_) => {
                    if !pending_sdk.remove(import.name()) {
                        return Err(Error::Compiler(format!("Module {} has a function that is not contemplated in the sdk", contract_path)));
                    }
                }
                _ => {
                    return Err(Error::Compiler(format!(
                        "Module {} has a import that is not function",
                        contract_path
                    )))
                }
            }
        }
        if !pending_sdk.is_empty() {
            return Err(Error::Compiler(format!(
                "Module {} has not al imports of sdk",
                contract_path
            )));
        }

        // We create a context from the state and the event.
        let (context, state_ptr) =
            Self::generate_context(state)?;

        // Container to store and manage the global state of a WebAssembly instance during its execution.
        let mut store = Store::new(&engine, context);

        // Responsible for combining several object files into a single WebAssembly executable file (.wasm).
        let linker = Self::generate_linker(&engine)?;

        // Contract instance.
        let instance =
            linker.instantiate(&mut store, &module).map_err(|e| {
                Error::Compiler(format!(
                    "Error when creating a contract instance: {}",
                    e
                ))
            })?;

        // Get access to contract, only to check if main_function exist.
        let _main_contract_entrypoint = instance
            .get_typed_func::<(u32, u32, u32), u32>(&mut store, "main_function")
            .map_err(|e| {
                Error::Runner(format!("Contract entry point not found: {}", e))
            })?;

        // Get access to contract
        let init_contract_entrypoint = instance
            .get_typed_func::<u32, u32>(&mut store, "init_check_function")
            .map_err(|e| {
                Error::Runner(format!("Contract entry point not found: {}", e))
            })?;
        
        // Contract execution
        let result_ptr = init_contract_entrypoint
            .call(
                &mut store,
                state_ptr,
            )
            .map_err(|e| {
                Error::Runner(format!("Contract execution failed: {}", e))
            })?;

        let result = Self::get_result(&store, result_ptr)?;
        
        if !result.success {
            todo!()
        }

        Ok(contract_bytes)
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

    // TODO SI todo funciona refactorizar este método que también está en el runner.
    // Cambiar errores De Runner a Compiler
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

    fn generate_context(state: ValueWrapper) -> Result<(MemoryManager, u32), Error> {
        let mut context = MemoryManager::new();
        let state_bytes = to_vec(&state).map_err(|e| {
            Error::Runner(format!(
                "Error when serializing the state using borsh: {}",
                e
            ))
        })?;
        let state_ptr = context.add_data_raw(&state_bytes);
        Ok((context, state_ptr as u32))
    }

    fn get_sdk_functions_identifier() -> HashSet<String> {
        HashSet::from_iter(vec![
            "alloc".to_owned(),
            "write_byte".to_owned(),
            "pointer_len".to_owned(),
            "read_byte".to_owned(),
        ])
    }
}

#[derive(Debug, Clone)]
pub enum CompilerMessage {
    Compile {
        contract: String,
        initial_value: Value,
        contract_path: String,
    },
    CompileCheck {
        contract: String,
        initial_value: Value,
        contract_path: String,
    },
}

impl Message for CompilerMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompilerEvent {}

impl Event for CompilerEvent {}

#[derive(Debug, Clone)]
pub enum CompilerResponse {
    Error(Error),
    Check,
    Ok,
}

impl Response for CompilerResponse {}

#[async_trait]
impl Actor for Compiler {
    type Event = CompilerEvent;
    type Message = CompilerMessage;
    type Response = CompilerResponse;
}

#[async_trait]
impl Handler<Compiler> for Compiler {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: CompilerMessage,
        _ctx: &mut ActorContext<Compiler>,
    ) -> Result<CompilerResponse, ActorError> {
        match msg {
            CompilerMessage::Compile {
                contract,
                contract_path,
                initial_value,
            } => {
                let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
                    *derivator
                } else {
                    error!("Error getting derivator");
                    DigestDerivator::Blake3_256
                };

                let contract_wrapper = ValueWrapper(json!({"raw": contract}));
                let contract_hash = match contract_wrapper.hash_id(derivator) {
                    Ok(hash) => hash,
                    Err(_e) => todo!(),
                };

                if contract_hash != self.contract {
                    if let Err(e) =
                        Self::compile_contract(&contract, &contract_path).await
                    {
                        return Ok(CompilerResponse::Error(e));
                    };

                    let contract = match Self::check_wasm(&contract_path, ValueWrapper(initial_value)).await
                    {
                        Ok(contract) => contract,
                        Err(e) => return Ok(CompilerResponse::Error(e)),
                    };

                    {
                        let mut contracts = CONTRACTS.write().await;
                        contracts.insert(contract_path.clone(), contract);
                    }

                    self.contract = contract_hash;
                }

                Ok(CompilerResponse::Ok)
            }
            CompilerMessage::CompileCheck {
                contract,
                contract_path,
                initial_value,
            } => {
                let derivator = if let Ok(derivator) = DIGEST_DERIVATOR.lock() {
                    *derivator
                } else {
                    error!("Error getting derivator");
                    DigestDerivator::Blake3_256
                };

                let contract_wrapper = ValueWrapper(json!({"raw": contract}));
                let contract_hash = match contract_wrapper.hash_id(derivator) {
                    Ok(hash) => hash,
                    Err(_e) => todo!(),
                };

                if contract_hash != self.contract {
                    if let Err(e) =
                        Self::compile_contract(&contract, &contract_path).await
                    {
                        return Ok(CompilerResponse::Error(e));
                    };

                    if let Err(e) = Self::check_wasm(&contract_path, ValueWrapper(initial_value)).await {
                        return Ok(CompilerResponse::Error(e));
                    }
                }

                Ok(CompilerResponse::Check)
            }
        }
    }
}
