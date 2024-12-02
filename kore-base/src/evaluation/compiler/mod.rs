use std::{collections::HashSet, path::Path, process::Command};

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_std::fs;
use async_trait::async_trait;
use base64::{prelude::BASE64_STANDARD, Engine as Base64Engine};
use borsh::{to_vec, BorshDeserialize, BorshSerialize};
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::error;
use wasmtime::{Caller, Config, Engine, ExternType, Linker, Module, Store};

use crate::{
    governance::json_schema::JsonSchema,
    model::common::{emit_fail, generate_linker, MemoryManager},
    Error, HashId, ValueWrapper, CONTRACTS, DIGEST_DERIVATOR,
};

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
    fn compilation_toml() -> String {
        r#"
    [package]
    name = "contract"
    version = "0.1.0"
    edition = "2021"
    
    [dependencies]
    serde = { version = "1.0.208", features = ["derive"] }
    serde_json = "1.0.125"
    json-patch = "3.0.1"
    thiserror = "2.0.3"
    kore-contract-sdk = { git = "https://github.com/kore-ledger/kore-contract-sdk.git", branch = "main"}
    
    [profile.release]
    strip = "debuginfo"
    lto = true
    
    [lib]
    crate-type = ["cdylib"]

    [workspace]
      "#
        .into()
    }

    async fn compile_contract(
        contract: &str,
        contract_path: &str,
    ) -> Result<(), Error> {
        // Write contract.
        let Ok(decode_base64) = BASE64_STANDARD.decode(contract) else {
            return Err(Error::Compiler(format!(
                "Failed to decode base64 {}",
                contract_path
            )));
        };

        if !Path::new(&format!("{}/src", contract_path)).exists() {
            fs::create_dir_all(&format!("{}/src", contract_path))
                .await
                .map_err(|e| {
                    Error::Node(format!("Can not create src dir: {}", e))
                })?;
        }

        let toml: String = Self::compilation_toml();
        // We write cargo.toml
        fs::write(format!("{}/Cargo.toml", contract_path), toml)
            .await
            .map_err(|e| {
                Error::Node(format!("Can not create Cargo.toml file: {}", e))
            })?;

        fs::write(
            format!("{}/src/lib.rs", contract_path),
            decode_base64,
        )
        .await
        .map_err(|e| {
            Error::Compiler(format!(
                "Can not create {}/src/lib.rs file: {}",
                contract_path, e
            ))
        })?;

        // Compiling contract
        let status = Command::new("cargo")
            .arg("build")
            .arg(format!(
                "--manifest-path={}/Cargo.toml",
                contract_path
            ))
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .arg("--release")
            .output()
            // Does not show stdout. Generates child process and waits
            .map_err(|e| {
                Error::Compiler(format!(
                    "Can not compile contract {}/src/lib.rs: {}",
                    contract_path, e
                ))
            })?;

        // Is success
        if !status.status.success() {
            return Err(Error::Compiler(format!(
                "Can not compile {}/src/lib.rs",
                contract_path
            )));
        }

        Ok(())
    }

    async fn check_wasm(
        contract_path: &str,
        state: ValueWrapper,
    ) -> Result<Vec<u8>, Error> {
        // Read compile contract
        let file = fs::read(format!(
            "{}/target/wasm32-unknown-unknown/release/contract.wasm",
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
                    Error::Compiler(format!(
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
        let (context, state_ptr) = Self::generate_context(state)?;

        // Container to store and manage the global state of a WebAssembly instance during its execution.
        let mut store = Store::new(&engine, context);

        // Responsible for combining several object files into a single WebAssembly executable file (.wasm).
        let linker = generate_linker(&engine)?;

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
                Error::Compiler(format!(
                    "Contract entry point not found: {}",
                    e
                ))
            })?;

        // Get access to contract
        let init_contract_entrypoint = instance
            .get_typed_func::<u32, u32>(&mut store, "init_check_function")
            .map_err(|e| {
                Error::Compiler(format!(
                    "Contract entry point not found: {}",
                    e
                ))
            })?;

        // Contract execution
        let result_ptr = init_contract_entrypoint
            .call(&mut store, state_ptr)
            .map_err(|e| {
            Error::Compiler(format!("Contract execution failed: {}", e))
        })?;

        Self::check_result(&store, result_ptr)?;

        Ok(contract_bytes)
    }

    fn check_result(
        store: &Store<MemoryManager>,
        pointer: u32,
    ) -> Result<(), Error> {
        let bytes = store.data().read_data(pointer as usize)?;
        let contract_result: ContractResult =
            BorshDeserialize::try_from_slice(bytes).map_err(|e| {
                Error::Compiler(format!(
                    "Can not generate wasm contract result: {}",
                    e
                ))
            })?;

        if contract_result.success {
            Ok(())
        } else {
            return Err(Error::Compiler(
                "Contract execution in compilation was not successful"
                    .to_owned(),
            ));
        }
    }

    fn generate_context(
        state: ValueWrapper,
    ) -> Result<(MemoryManager, u32), Error> {
        let mut context = MemoryManager::new();
        let state_bytes = to_vec(&state).map_err(|e| {
            Error::Compiler(format!(
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
        contract_name: String,
        initial_value: Value,
        contract_path: String,
    },
}

impl Message for CompilerMessage {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompilerEvent {}

impl Event for CompilerEvent {}

#[async_trait]
impl Actor for Compiler {
    type Event = CompilerEvent;
    type Message = CompilerMessage;
    type Response = ();
}

#[async_trait]
impl Handler<Compiler> for Compiler {
    async fn handle_message(
        &mut self,
        _sender: ActorPath,
        msg: CompilerMessage,
        _ctx: &mut ActorContext<Compiler>,
    ) -> Result<(), ActorError> {
        match msg {
            CompilerMessage::Compile {
                contract,
                contract_name,
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
                    Err(e) => {
                        return Err(ActorError::Functional(format!(
                            "Can not hash contract: {}",
                            e.to_string()
                        )))
                    }
                };

                if contract_hash != self.contract {
                    if let Err(e) =
                        Self::compile_contract(&contract, &contract_path).await
                    {
                        return Err(ActorError::Functional(e.to_string()));
                    };

                    let contract = match Self::check_wasm(
                        &contract_path,
                        ValueWrapper(initial_value),
                    )
                    .await
                    {
                        Ok(contract) => contract,
                        Err(e) => {
                            return Err(ActorError::Functional(e.to_string()))
                        }
                    };

                    {
                        let mut contracts = CONTRACTS.write().await;
                        contracts.insert(contract_name.clone(), contract);
                    }

                    self.contract = contract_hash;
                }

                Ok(())
            }
        }
    }
}
