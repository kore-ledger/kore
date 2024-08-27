use std::{collections::HashSet, process::Command};

use actor::{
    Actor, ActorContext, Error as ActorError, Event, Handler, Message, Response,
};
use async_std::fs;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use wasmtime::{Config, Engine, ExternType, Module};

use crate::Error;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Compiler {}

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
            .arg(format!("--manifest-path=contracts/Cargo.toml"))
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

    async fn check_wasm(contract_path: &str) -> Result<Vec<u8>, Error> {
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
            Module::deserialize(&engine, contract_bytes.clone()).map_err(|e| {
                Error::Runner(format!(
                    "Error deserializing the contract in wastime: {}",
                    e
                ))
            })?
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
                _ => return Err(Error::Compiler(format!("Module {} has a import that is not function", contract_path))),
            }
        }
        if !pending_sdk.is_empty() {
            return Err(Error::Compiler(format!("Module {} has not al imports of sdk", contract_path)))
        }

        Ok(contract_bytes)
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
pub enum CompilerCommand {
    Compile{
        contract: String,
        contract_path: String
    },
    CompileCheck{
        contract: String,
        contract_path: String
    },
}

impl Message for CompilerCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompilerEvent {}

impl Event for CompilerEvent {}

#[derive(Debug, Clone)]
pub enum CompilerResponse {
    Error(Error),
    Check,
    Compilation(Vec<u8>)
}

impl Response for CompilerResponse {}

#[async_trait]
impl Actor for Compiler {
    type Event = CompilerEvent;
    type Message = CompilerCommand;
    type Response = CompilerResponse;
}

#[async_trait]
impl Handler<Compiler> for Compiler {
    async fn handle_message(
        &mut self,
        msg: CompilerCommand,
        _ctx: &mut ActorContext<Compiler>,
    ) -> Result<CompilerResponse, ActorError> {
        match msg {
            CompilerCommand::Compile { contract, contract_path } => {
                if let Err(e) = Self::compile_contract(&contract, &contract_path).await {
                    return Ok(CompilerResponse::Error(e));
                };

                match Self::check_wasm(&contract_path).await {
                    Ok(wasm) => Ok(CompilerResponse::Compilation(wasm)),
                    Err(e) => Ok(CompilerResponse::Error(e))
                }
            },
            CompilerCommand::CompileCheck { contract, contract_path } => {
                if let Err(e) = Self::compile_contract(&contract, &contract_path).await {
                    return Ok(CompilerResponse::Error(e));
                };

                match Self::check_wasm(&contract_path).await {
                    Ok(_) => Ok(CompilerResponse::Check),
                    Err(e) => Ok(CompilerResponse::Error(e))
                }
            }
        }
    }
}
