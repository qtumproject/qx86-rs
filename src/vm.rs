use std::collections::HashMap;
use crate::pipeline::*;

#[allow(dead_code)] //remove after design stuff is done

#[derive(Default)]
pub struct VM{
    pub regs: [u32; 8], //EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI
    pub eip: u32,
    pub eflags: u32,

    pub memory: MemorySystem,
    pub pipeline: Vec<Pipeline>
    //todo: hypervisor to call external code
}

#[derive(Default)]
pub struct BufferMemory{
    pub memory: Vec<u8>,
    pub readonly: bool
}

#[derive(Default)]
pub struct MemorySystem{
    map: HashMap<u32, BufferMemory>
}

#[derive(PartialEq, Debug)]
#[derive(Copy, Clone)]
pub enum MemoryError{
    NoExist,
    UnalignedAdd,
    ConflictingAdd,
    Overrun
}

impl MemorySystem{
    pub fn add_memory(&mut self, address: u32, size: u32, readonly: bool) -> Result<&mut [u8], MemoryError> {
        if address & 0xFFFF != 0{
            return Err(MemoryError::UnalignedAdd);
        }
        
        let aligned = address & 0xFFFF0000;
        if self.map.contains_key(&aligned) {
            return Err(MemoryError::ConflictingAdd);
        }
        let mut b = BufferMemory{
            memory: Vec::default(),
            readonly: readonly
        };
        b.memory.resize(size as usize, 0);
        self.map.insert(aligned, b);
        self.map.get_mut(&aligned).unwrap().memory[0] = 10;
        Ok(&mut self.map.get_mut(&aligned).unwrap().memory[0..])
    }

    pub fn get_mut_memory(&mut self, address: u32) -> Result<&mut [u8], MemoryError> {
        match self.map.get_mut(&(address & 0xFFFF0000)){
            Option::None => return Err(MemoryError::NoExist),
            Option::Some(m) =>  {
                let local = (address & 0xFFFF) as usize;
                if m.memory.len() - 1 < local{
                    return Err(MemoryError::Overrun);
                }
                return Ok(&mut (&mut m.memory)[local..])
            }
        }
    }
    pub fn get_memory(&self, address: u32) -> Result<&[u8], MemoryError> {
        match self.map.get(&(address & 0xFFFF0000)){
            Option::None => return Err(MemoryError::NoExist),
            Option::Some(m) =>  {
                let local = (address & 0xFFFF) as usize;
                if m.memory.len() - 1 < local{
                    return Err(MemoryError::Overrun);
                }
                return Ok(&(&m.memory)[local..])
            }
        }
    }
    pub fn section_exists(&self, address: u32) -> bool{
        self.map.contains_key(&(address & 0xFFFF0000))
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn test_memory(){
        let mut m = MemorySystem::default();
        let bytes = m.add_memory(0x10000, 0x100, false).unwrap();
        bytes[0x10] = 0x12;
        assert!(m.get_memory(0x10010).unwrap()[0] == 0x12);
        let mb = m.get_mut_memory(0x10020).unwrap();
        mb[0] = 0x34;
        assert!(m.get_memory(0x10020).unwrap()[0] == 0x34);
        assert!(m.section_exists(0x10000));
        assert!(!m.section_exists(0x20000));
    }
    #[test]
    fn test_memory_failures(){
        let mut m = MemorySystem::default();
        let bytes = m.add_memory(0x10000, 0x100, false).unwrap();
        assert!(m.add_memory(0x10000, 0x100, false) == Err(MemoryError::ConflictingAdd));
        assert!(m.add_memory(0x100FF, 0x100, false) == Err(MemoryError::UnalignedAdd));
        assert!(m.get_memory(0x10200) == Err(MemoryError::Overrun));
        assert!(m.get_mut_memory(0x10100) == Err(MemoryError::Overrun));
    }
}
