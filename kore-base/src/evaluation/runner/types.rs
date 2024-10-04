use std::collections::HashMap;

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    governance::{Member, Policy, Role, Schema},
    Error, ValueWrapper,
};

#[derive(Debug)]
pub struct MemoryManager {
    memory: Vec<u8>,
    map: HashMap<usize, usize>,
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            memory: vec![],
            map: HashMap::new(),
        }
    }

    pub fn alloc(&mut self, len: usize) -> usize {
        let current_len = self.memory.len();
        self.memory.resize(current_len + len, 0);
        self.map.insert(current_len, len);
        current_len
    }

    pub fn write_byte(&mut self, start_ptr: usize, offset: usize, data: u8) {
        self.memory[start_ptr + offset] = data;
    }

    pub fn read_byte(&self, ptr: usize) -> u8 {
        self.memory[ptr]
    }

    pub fn read_data(&self, ptr: usize) -> Result<&[u8], Error> {
        let len = self
            .map
            .get(&ptr)
            .ok_or(Error::Runner("Invalid pointer provided".to_owned()))?;
        Ok(&self.memory[ptr..ptr + len])
    }

    pub fn get_pointer_len(&self, ptr: usize) -> isize {
        let Some(result) = self.map.get(&ptr) else {
            return -1;
        };
        *result as isize
    }

    pub fn add_data_raw(&mut self, bytes: &[u8]) -> usize {
        let ptr = self.alloc(bytes.len());
        for (index, byte) in bytes.iter().enumerate() {
            self.memory[ptr + index] = *byte;
        }
        ptr
    }
}

#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone,
)]
pub struct ContractResult {
    pub final_state: ValueWrapper,
    pub success: bool,
}

#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone,
)]
pub struct RunnerResult {
    pub final_state: ValueWrapper,
    pub approval_required: bool,
    pub success: bool,
}


#[derive(Debug, Clone)]
pub enum Contract {
    CompiledContract(Vec<u8>),
    GovContract,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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
