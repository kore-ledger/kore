use serde::{Deserialize, Serialize};

use crate::{governance::{Member, Policy, Role, Schema}, ValueWrapper};


#[derive(Serialize, Deserialize, Debug)]
pub struct ContractResult {
    pub final_state: ValueWrapper,
    pub approval_required: bool,
    pub success: bool,
}

impl ContractResult {
    pub fn error() -> Self {
        Self {
            final_state: ValueWrapper(serde_json::Value::Null),
            approval_required: false,
            success: false,
        }
    }
}

pub enum Contract {
    CompiledContract(Vec<u8>),
    GovContract,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GovernanceData {
    pub members: Vec<Member>,
    pub roles: Vec<Role>,
    pub schemas: Vec<Schema>,
    pub policies: Vec<Policy>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum GovernanceEvent {
    Patch { data: ValueWrapper },
}
