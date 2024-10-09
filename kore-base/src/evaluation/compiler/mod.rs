use std::{collections::HashSet, process::Command};

use actor::{
    Actor, ActorContext, ActorPath, Error as ActorError, Event, Handler,
    Message, Response,
};
use async_std::fs;
use async_trait::async_trait;
use identity::identifier::{derive::digest::DigestDerivator, DigestIdentifier};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::error;
use wasmtime::{Config, Engine, ExternType, Module};

use crate::{
    governance::json_schema::JsonSchema, Error, HashId, ValueWrapper,
    CONTRACTS, DIGEST_DERIVATOR, SCHEMAS,
};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Compiler {
    pub schema: DigestIdentifier,
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
pub enum CompilerMessage {
    Compile {
        contract: String,
        schema: ValueWrapper,
        initial_value: Value,
        contract_path: String,
    },
    CompileCheck {
        contract: String,
        schema: ValueWrapper,
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
        sender: ActorPath,
        msg: CompilerMessage,
        _ctx: &mut ActorContext<Compiler>,
    ) -> Result<CompilerResponse, ActorError> {
        match msg {
            CompilerMessage::Compile {
                contract,
                contract_path,
                initial_value,
                schema,
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
                    Err(e) => todo!(),
                };

                if contract_hash != self.contract {
                    if let Err(e) =
                        Self::compile_contract(&contract, &contract_path).await
                    {
                        return Ok(CompilerResponse::Error(e));
                    };

                    let contract = match Self::check_wasm(&contract_path).await
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

                let schema_hash = match schema.hash_id(derivator) {
                    Ok(hash) => hash,
                    Err(e) => todo!(),
                };

                if schema_hash != self.schema {
                    let compilation = match JsonSchema::compile(&schema.0) {
                        Ok(compilation) => compilation,
                        Err(e) => todo!(),
                    };

                    if !compilation.fast_validate(&initial_value) {
                        todo!()
                    }

                    {
                        let mut schemas = SCHEMAS.write().await;
                        schemas.insert(contract_path, compilation);
                    }
                    self.schema = schema_hash;
                }

                Ok(CompilerResponse::Ok)
            }
            CompilerMessage::CompileCheck {
                contract,
                contract_path,
                initial_value,
                schema,
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
                    Err(e) => todo!(),
                };

                if contract_hash != self.contract {
                    if let Err(e) =
                        Self::compile_contract(&contract, &contract_path).await
                    {
                        return Ok(CompilerResponse::Error(e));
                    };

                    if let Err(e) = Self::check_wasm(&contract_path).await {
                        return Ok(CompilerResponse::Error(e));
                    }
                }

                let schema_hash = match schema.hash_id(derivator) {
                    Ok(hash) => hash,
                    Err(e) => todo!(),
                };

                if schema_hash != self.schema {
                    let compilation = match JsonSchema::compile(&schema.0) {
                        Ok(compilation) => compilation,
                        Err(e) => todo!(),
                    };

                    if !compilation.fast_validate(&initial_value) {
                        todo!()
                    }
                }

                Ok(CompilerResponse::Check)
            }
        }
    }
}
