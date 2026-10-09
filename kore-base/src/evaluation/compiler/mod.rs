use std::{collections::HashSet, path::Path, process::Command};

use async_trait::async_trait;
use base64::{Engine as Base64Engine, prelude::BASE64_STANDARD};
use borsh::{BorshDeserialize, BorshSerialize, to_vec};
use identity::identifier::{DigestIdentifier, derive::digest::DigestDerivator};
use rush::{
    Actor, ActorContext, ActorError, ActorPath, Event, Handler, Message,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::fs;

use tracing::error;
use wasmtime::{Config, Engine, ExternType, Module, Store};

use crate::{
    CONTRACTS, DIGEST_DERIVATOR, Error, HashId, ValueWrapper,
    model::common::{MemoryManager, generate_linker},
};

const TARGET_COMPILER: &str = "Kore-Evaluation-Compiler";

#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone,
)]
pub struct ContractResult {
    pub success: bool,
    pub error: String,
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
    edition = "2024"
    
    [dependencies]
    serde = { version = "1.0.219", features = ["derive"] }
    serde_json = "1.0.140"
    kore-contract-sdk = "0.6.0"
    
    [profile.release]
    strip = "debuginfo"
    lto = true
    
    [lib]
    crate-type = ["cdylib"]

    [workspace]
      "#
        .into()
    }

    fn config_toml() -> String {
        r#"
    [target.wasm32-unknown-unknown]
    rustflags = ["-C", "link-arg=--allow-undefined"]
        "#.into()
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
        if !Path::new(&format!("{}/.cargo", contract_path)).exists() {
            fs::create_dir_all(&format!("{}/.cargo", contract_path))
                .await
                .map_err(|e| {
                    Error::Node(format!("Can not create .cargo dir: {}", e))
            })?;
        }

        // We write cargo.toml
        let toml: String = Self::compilation_toml();
        fs::write(format!("{}/Cargo.toml", contract_path), toml)
            .await
            .map_err(|e| {
                Error::Node(format!("Can not create Cargo.toml file: {}", e))
            })?;

        // We write cofig.toml
        let config = Self::config_toml();
        fs::write(format!("{}/.cargo/config.toml", contract_path), config)
            .await
            .map_err(|e| {
                Error::Node(format!("Can not create config.toml file: {}", e))
            })?;

        fs::write(format!("{}/src/lib.rs", contract_path), decode_base64)
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
            .arg(format!("--manifest-path={}/Cargo.toml", contract_path))
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
                        return Err(Error::Compiler(format!(
                            "Module {} has a function that is not contemplated in the sdk",
                            contract_path
                        )));
                    }
                }
                _ => {
                    return Err(Error::Compiler(format!(
                        "Module {} has a import that is not function",
                        contract_path
                    )));
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
            .get_typed_func::<(u32, u32, u32, u32), u32>(
                &mut store,
                "main_function",
            )
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
            Err(Error::Compiler(format!(
                "Contract execution in compilation was not successful: {}",
                contract_result.error
            )))
        }
    }

    fn generate_context(
        state: ValueWrapper,
    ) -> Result<(MemoryManager, u32), Error> {
        let mut context = MemoryManager::default();
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
                    error!(TARGET_COMPILER, "Error getting derivator");
                    DigestDerivator::Blake3_256
                };

                let contract_wrapper = ValueWrapper(json!({"raw": contract}));
                let contract_hash = match contract_wrapper.hash_id(derivator) {
                    Ok(hash) => hash,
                    Err(e) => {
                        error!(
                            TARGET_COMPILER,
                            "Compile, Can not hash contract: {}", e
                        );
                        return Err(ActorError::Functional(format!(
                            "Can not hash contract: {}",
                            e
                        )));
                    }
                };

                if contract_hash != self.contract {
                    if let Err(e) =
                        Self::compile_contract(&contract, &contract_path).await
                    {
                        error!(
                            TARGET_COMPILER,
                            "Compile, Can not compile: {}", e
                        );
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
                            error!(
                                TARGET_COMPILER,
                                "Compile, Can check wasm: {}", e
                            );
                            return Err(ActorError::Functional(e.to_string()));
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

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_compilation() {
        //let compiler = Compiler { contract: DigestIdentifier::default() };
        let contract = "Ly8KCnVzZSBzZXJkZTo6e1NlcmlhbGl6ZSwgRGVzZXJpYWxpemV9Owp1c2Uga29yZV9jb250cmFjdF9zZGsgYXMgc2RrOwoKLy8vIERlZmluZSB0aGUgc3RhdGUgb2YgdGhlIGNvbnRyYWN0LiAKI1tkZXJpdmUoU2VyaWFsaXplLCBEZXNlcmlhbGl6ZSwgQ2xvbmUpXQpzdHJ1Y3QgU3RhdGUgewogICAgcHViIGNvdW50ZXI6IHUzMiwKfQoKLy8vIERlZmluZSB0aGUgZXZlbnRzIHRoYXQgY2FuIGJlIGVtaXR0ZWQgYnkgdGhlIGNvbnRyYWN0LgojW2Rlcml2ZShTZXJpYWxpemUsIERlc2VyaWFsaXplLCBDbG9uZSldCmVudW0gRXZlbnQgewogICAgSW5jcmVtZW50ZWQsCiAgICBEZWNyZW1lbnRlZCwKfQoKI1t1bnNhZmUobm9fbWFuZ2xlKV0KcHViIHVuc2FmZSBmbiBtYWluX2Z1bmN0aW9uKHN0YXRlX3B0cjogaTMyLCBpbml0X3N0YXRlX3B0cjogaTMyLCBldmVudF9wdHI6IGkzMiwgaXNfb3duZXI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmV4ZWN1dGVfY29udHJhY3Qoc3RhdGVfcHRyLCBpbml0X3N0YXRlX3B0ciwgZXZlbnRfcHRyLCBpc19vd25lciwgY29udHJhY3RfbG9naWMpCn0KCiNbdW5zYWZlKG5vX21hbmdsZSldCnB1YiB1bnNhZmUgZm4gaW5pdF9jaGVja19mdW5jdGlvbihzdGF0ZV9wdHI6IGkzMikgLT4gdTMyIHsKICBzZGs6OmNoZWNrX2luaXRfZGF0YShzdGF0ZV9wdHIsIGluaXRfbG9naWMpCn0KCmZuIGluaXRfbG9naWMoCiAgX3N0YXRlOiAmU3RhdGUsCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RJbml0Q2hlY2ssCikgewogIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQoKZm4gY29udHJhY3RfbG9naWMoCiAgY29udGV4dDogJnNkazo6Q29udGV4dDxFdmVudD4sCiAgY29udHJhY3RfcmVzdWx0OiAmbXV0IHNkazo6Q29udHJhY3RSZXN1bHQ8U3RhdGU+LAopIHsKICAgIGxldCBzdGF0ZSA9ICZtdXQgY29udHJhY3RfcmVzdWx0LnN0YXRlOwogICAgbWF0Y2ggY29udGV4dC5ldmVudCB7CiAgICAgICAgRXZlbnQ6OkluY3JlbWVudGVkID0+IHN0YXRlLmNvdW50ZXIgKz0gMSwKICAgICAgICBFdmVudDo6RGVjcmVtZW50ZWQgPT4gewogICAgICAgICAgICBpZiBzdGF0ZS5jb3VudGVyID09IDAgewogICAgICAgICAgICAgICAgY29udHJhY3RfcmVzdWx0LnN1Y2Nlc3MgPSBmYWxzZTsKICAgICAgICAgICAgICAgIGNvbnRyYWN0X3Jlc3VsdC5lcnJvciA9ICJDb3VudGVyIGNhbm5vdCBiZSBkZWNyZW1lbnRlZCBiZWxvdyB6ZXJvLiIudG9fb3duZWQoKTsKICAgICAgICAgICAgICAgIHJldHVybjsKICAgICAgICAgICAgfQogICAgICAgICAgICBzdGF0ZS5jb3VudGVyIC09IDEKICAgICAgICB9LAogICAgfQoKICAgIGNvbnRyYWN0X3Jlc3VsdC5zdWNjZXNzID0gdHJ1ZTsKfQo=";
        let contract_path = "./contracts";
        let result = Compiler::compile_contract(contract, contract_path).await;
        assert!(result.is_ok());
    }
}