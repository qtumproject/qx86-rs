use std::collections::HashMap;

use crate::vm::*;

pub const READONLY_MEMORY:u32 = 0x80000000;

#[derive(Default)]
pub struct BufferMemory{
    pub memory: Vec<u8>,
}

#[derive(Default)]
pub struct MemorySystem{
    map: HashMap<u32, BufferMemory>
}

impl MemorySystem{
    pub fn add_memory(&mut self, address: u32, size: u32) -> Result<&mut [u8], VMError> {
        if address & 0xFFFF != 0{
            return Err(VMError::UnalignedMemoryAddition);
        }
        
        let aligned = address & 0xFFFF0000;
        if self.map.contains_key(&aligned) {
            return Err(VMError::ConflictingMemoryAddition);
        }
        let mut b = BufferMemory{
            memory: Vec::default(),
        };
        b.memory.resize(size as usize, 0);
        self.map.insert(aligned, b);
        self.map.get_mut(&aligned).unwrap().memory[0] = 10;
        Ok(&mut self.map.get_mut(&aligned).unwrap().memory[0..])
    }

    //Note that this will not respect the "readonly" flag, nor readonly memory space
    //This is designed for internal use and with the VM exposed methods checking for these errors
    pub fn get_mut_memory(&mut self, address: u32) -> Result<&mut [u8], VMError> {
        match self.map.get_mut(&(address & 0xFFFF0000)){
            Option::None => return Err(VMError::ReadUnloadedMemory(address)), //should never happen?
            Option::Some(m) =>  {
                let local = (address & 0xFFFF) as usize;
                if m.memory.len() - 1 < local{
                    return Err(VMError::WroteBadMemory(address));
                }
                return Ok(&mut (&mut m.memory)[local..])
            }
        }
    }
    pub fn get_memory(&self, address: u32) -> Result<&[u8], VMError> {
        match self.map.get(&(address & 0xFFFF0000)){
            Option::None => return Err(VMError::ReadUnloadedMemory(address)),
            Option::Some(m) =>  {
                let local = (address & 0xFFFF) as usize;
                if m.memory.len() - 1 < local{
                    return Err(VMError::ReadBadMemory(address));
                }
                return Ok(&(&m.memory)[local..])
            }
        }
    }
    pub fn get_sized_memory(&self, address: u32, size: u32) -> Result<&[u8], VMError>{
        let m = self.get_memory(address)?;
        if m.len() < size as usize {
            return Err(VMError::ReadBadMemory(address + size - 1));
        }
        Ok(m)
    }
    pub fn get_mut_sized_memory(&mut self, address: u32, size: u32) -> Result<&mut [u8], VMError>{
        let m = self.get_mut_memory(address)?;
        if m.len() < size as usize {
            return Err(VMError::WroteBadMemory(address + size - 1));
        }
        Ok(m)
    }
    pub fn get_u8(&self, address: u32) -> Result<u8, VMError>{
        let m = self.get_sized_memory(address, 1)?;
        Ok(m[0])
    }
    pub fn get_u16(&self, address: u32) -> Result<u16, VMError>{
        use std::convert::TryInto;
        let m = self.get_sized_memory(address, 2)?;
        let v: [u8; 2] = *(&m[0..2].try_into().unwrap());
        Ok(u16::from_le_bytes(v))
    }
    pub fn get_u32(&self, address: u32) -> Result<u32, VMError>{
        use std::convert::TryInto;
        let m = self.get_sized_memory(address, 4)?;
        let v: [u8; 4] = *(&m[0..4].try_into().unwrap());
        Ok(u32::from_le_bytes(v))
    }
    pub fn set_u8(&mut self, address: u32, v: u8) -> Result<u8, VMError>{
        let m = self.get_mut_sized_memory(address, 1)?;
        m[0] = v;
        Ok(v)
    }
    pub fn set_u16(&mut self, address: u32, v: u16) -> Result<u16, VMError>{
        let m = self.get_mut_sized_memory(address, 2)?;
        let d = v.to_le_bytes();
        (&mut m[0..2]).copy_from_slice(&d);
        Ok(v)
    }
    pub fn set_u32(&mut self, address: u32, v: u32) -> Result<u32, VMError>{
        let m = self.get_mut_sized_memory(address, 4)?;
        let d = v.to_le_bytes();
        (&mut m[0..4]).copy_from_slice(&d);
        Ok(v)
    }
    pub fn section_exists(&self, address: u32) -> bool{
        self.map.contains_key(&(address & 0xFFFF0000))
    }
}