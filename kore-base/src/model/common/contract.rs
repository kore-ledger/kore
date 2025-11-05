use std::collections::HashMap;
use wasmtime::{Caller, Engine, Linker};

use crate::Error;

#[derive(Debug, Default)]
pub struct MemoryManager {
    memory: Vec<u8>,
    map: HashMap<usize, usize>,
}

impl MemoryManager {
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

pub fn generate_linker(
    engine: &Engine,
) -> Result<Linker<MemoryManager>, Error> {
    let mut linker: Linker<MemoryManager> = Linker::new(engine);

    // functions are created for webasembly modules, the logic of which is programmed in Rust
    linker
        .func_wrap(
            "env",
            "pointer_len",
            |caller: Caller<'_, MemoryManager>, pointer: i32| {
                caller.data().get_pointer_len(pointer as usize)
                    as u32
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: pointer_len, {}", e))
        })?;

    linker
        .func_wrap(
            "env",
            "alloc",
            |mut caller: Caller<'_, MemoryManager>, len: u32| {
                caller.data_mut().alloc(len as usize) as u32
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: allow, {}", e))
        })?;

    linker
        .func_wrap(
            "env",
            "write_byte",
            |mut caller: Caller<'_, MemoryManager>, ptr: u32, offset: u32, data: u32| {
                caller
                    .data_mut()
                    .write_byte(ptr as usize, offset as usize, data as u8)
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: write_byte, {}", e))
        })?;

    linker
        .func_wrap(
            "env",
            "read_byte",
            |caller: Caller<'_, MemoryManager>, index: i32| {
                caller.data().read_byte(index as usize) as u32
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: read_byte, {}", e))
        })?;

    // TODO quitar?
    linker
        .func_wrap(
            "env",
            "cout",
            |_caller: Caller<'_, MemoryManager>, _ptr: u32| {
            },
        )
        .map_err(|e| {
            Error::Compiler(format!("An error has occurred linking a function, module: env, name: cout, {}", e))
        })?;
    Ok(linker)
}
