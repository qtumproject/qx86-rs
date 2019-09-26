use std::collections::HashMap;

use crate::vm::*;

/// Any virtual address equal to or greater than this value will be considered writeable
/// Any virtual address less than this will be considered read only
pub const WRITEABLE_MEMORY:u32 = 0x80000000;

/// A simple buffer of memory for MemorySystem
#[derive(Default)]
pub struct BufferMemory{
    pub memory: Vec<u8>,
}
/// The system for tracking all memory within the VM
#[derive(Default)]
pub struct MemorySystem{
    map: HashMap<u32, BufferMemory>
}

impl MemorySystem{
    /// This adds a new block of memory to the current memory system
    /// Note that the maximum size allowed is 0x10000 and the address must be aligned on an 0x10000 byte scale (ie, 64Kb)
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

    /// Note that this will not respect the "readonly" flag, nor readonly memory space
    /// This is designed for internal use and with the VM exposed methods checking for these errors
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
    /// This will get an area of memory as a slice of bytes
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
    /// This will get an area of memory as a slice of bytes and will return an error if the size requested is not available
    pub fn get_sized_memory(&self, address: u32, size: u32) -> Result<&[u8], VMError>{
        let m = self.get_memory(address)?;
        if m.len() < size as usize {
            return Err(VMError::ReadBadMemory(address + size - 1));
        }
        Ok(&m[0..size as usize])
    }
    /// This will get an area of mutable memory as a slice of bytes and will return an error if the size requested is not available
    /// Note that this will not respect the "readonly" flag, nor readonly memory space
    /// This is designed for internal use and with the VM exposed methods checking for these errors
    pub fn get_mut_sized_memory(&mut self, address: u32, size: u32) -> Result<&mut [u8], VMError>{
        let m = self.get_mut_memory(address)?;
        if m.len() < size as usize {
            return Err(VMError::WroteBadMemory(address + size - 1));
        }
        Ok(&mut m[0..size as usize])
    }
    /// Retreives a single u8 from memory
    pub fn get_u8(&self, address: u32) -> Result<u8, VMError>{
        let m = self.get_sized_memory(address, 1)?;
        Ok(m[0])
    }
    /// Retreives a single u16 from memory, including endianness correction if needed
    pub fn get_u16(&self, address: u32) -> Result<u16, VMError>{
        use std::convert::TryInto;
        let m = self.get_sized_memory(address, 2)?;
        let v: [u8; 2] = *(&m[0..2].try_into().unwrap());
        Ok(u16::from_le_bytes(v))
    }
    /// Retreives a single u32 from memory, including endianness correction if needed
    pub fn get_u32(&self, address: u32) -> Result<u32, VMError>{
        use std::convert::TryInto;
        let m = self.get_sized_memory(address, 4)?;
        let v: [u8; 4] = *(&m[0..4].try_into().unwrap());
        Ok(u32::from_le_bytes(v))
    }
    /// Sets a single u8 in memory
    pub fn set_u8(&mut self, address: u32, v: u8) -> Result<u8, VMError>{
        let m = self.get_mut_sized_memory(address, 1)?;
        m[0] = v;
        Ok(v)
    }
    /// Sets a single u16 in memory, including endianness correction if needed
    pub fn set_u16(&mut self, address: u32, v: u16) -> Result<u16, VMError>{
        let m = self.get_mut_sized_memory(address, 2)?;
        let d = v.to_le_bytes();
        (&mut m[0..2]).copy_from_slice(&d);
        Ok(v)
    }
    /// Sets a single u32 in memory, including endianness correction if needed
    pub fn set_u32(&mut self, address: u32, v: u32) -> Result<u32, VMError>{
        let m = self.get_mut_sized_memory(address, 4)?;
        let d = v.to_le_bytes();
        (&mut m[0..4]).copy_from_slice(&d);
        Ok(v)
    }
    /// Determines if a block of memory exists
    pub fn section_exists(&self, address: u32) -> bool{
        self.map.contains_key(&(address & 0xFFFF0000))
    }
}