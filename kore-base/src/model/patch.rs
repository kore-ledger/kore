use json_patch::{patch, Patch};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum PatchErrors {
    #[error("JSON provided is not of patch type: {0}")]
    JsonIsNotPatch(String),
    #[error("Error generating the Patch: {0}")]
    PatchGenerationError(String),
    #[error("Error expressing patch as JSON: {0}")]
    PatchToJsonFailed(String),
}

pub fn apply_patch<State: for<'a> Deserialize<'a> + Serialize>(
    patch_arg: Value,
    mut state: Value,
) -> Result<State, PatchErrors> {
    let patch_data: Patch = serde_json::from_value(patch_arg)
        .map_err(|e| PatchErrors::JsonIsNotPatch(e.to_string()))?;
    patch(&mut state, &patch_data)
        .map_err(|e| PatchErrors::PatchGenerationError(e.to_string()))?;
    serde_json::from_value(state)
        .map_err(|e| PatchErrors::PatchToJsonFailed(e.to_string()))
}
