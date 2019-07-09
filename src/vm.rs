use std::collections::HashMap;
use crate::pipeline::*;
use crate::opcodes::*;
use crate::structs::*;

#[allow(dead_code)] //remove after design stuff is done

#[derive(Default)]
pub struct VM{
    pub regs: [u32; 8], //EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI
    pub eip: u32,
    pub eflags: u32,

    pub memory: MemorySystem,
    pub pipeline: Vec<Pipeline>,
    //todo: hypervisor to call external code

    //set when an error has occurred within an opcode execution
    pub errored: Option<OpcodeError>,
    pub error_eip: u32
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
    pub fn get_sized_memory(&self, address: u32, size: u32) -> Result<&[u8], MemoryError>{
        let m = self.get_memory(address)?;
        if m.len() < size as usize {
            return Err(MemoryError::Overrun);
        }
        Ok(m)
    }
    pub fn get_mut_sized_memory(&mut self, address: u32, size: u32) -> Result<&mut [u8], MemoryError>{
        let m = self.get_mut_memory(address)?;
        if m.len() < size as usize {
            return Err(MemoryError::Overrun);
        }
        Ok(m)
    }
    pub fn get_u8(&self, address: u32) -> Result<u8, MemoryError>{
        let m = self.get_sized_memory(address, 1)?;
        Ok(m[0])
    }
    pub fn get_u16(&self, address: u32) -> Result<u16, MemoryError>{
        use std::convert::TryInto;
        let m = self.get_sized_memory(address, 2)?;
        let v: [u8; 2] = *(&m[0..2].try_into().unwrap());
        Ok(u16::from_le_bytes(v))
    }
    pub fn get_u32(&self, address: u32) -> Result<u32, MemoryError>{
        use std::convert::TryInto;
        let m = self.get_sized_memory(address, 4)?;
        let v: [u8; 4] = *(&m[0..4].try_into().unwrap());
        Ok(u32::from_le_bytes(v))
    }
    pub fn set_u8(&mut self, address: u32, v: u8) -> Result<u8, MemoryError>{
        let m = self.get_mut_sized_memory(address, 1)?;
        m[0] = v;
        Ok(v)
    }
    pub fn set_u16(&mut self, address: u32, v: u16) -> Result<u16, MemoryError>{
        let m = self.get_mut_sized_memory(address, 2)?;
        let d = v.to_le_bytes();
        (&mut m[0..2]).copy_from_slice(&d);
        Ok(v)
    }
    pub fn set_u32(&mut self, address: u32, v: u32) -> Result<u32, MemoryError>{
        let m = self.get_mut_sized_memory(address, 4)?;
        let d = v.to_le_bytes();
        (&mut m[0..4]).copy_from_slice(&d);
        Ok(v)
    }
    pub fn section_exists(&self, address: u32) -> bool{
        self.map.contains_key(&(address & 0xFFFF0000))
    }
}

impl VM{
    pub fn get_arg(&self, arg: ArgLocation) -> SizedValue{
        use ArgLocation::*;
        match arg{
            None => SizedValue::None,
            Immediate(v) => v,
            Address(a, s) => {
                SizedValue::None
            },
            RegisterValue(r, s) => {
                self.get_reg(r, s)
            },
            RegisterAddress(r, s) => {
                SizedValue::None
            },
            ModRMAddress16{offset, reg1, reg2, size} => {
                SizedValue::None
            },
            ModRMAddress32{offset, reg, size} => {
                SizedValue::None
            },
            SIBAddress32{offset, base, scale, index, size} => {
                SizedValue::None
            }
        }
    }
    pub fn get_reg(&self, reg: u8, size: ValueSize) -> SizedValue{
        use ValueSize::*;
        let r = reg as usize;
        match size{
            ValueSize::None => SizedValue::None,
            Byte => {
                if reg & 0x04 == 0{
                    //access lows, AL, CL, DL, BL
                    SizedValue::Byte((self.regs[r] & 0xFF) as u8)
                }else{
                    //access highs, AH, CH, DH, BH
                    SizedValue::Byte(((self.regs[r & 0x03] & 0xFF00) >> 8) as u8)
                }
            },
            Word => {
                SizedValue::Word((self.regs[r] & 0xFFFF) as u16)
            },
            Dword => {
                SizedValue::Dword(self.regs[r])
            }
        }
    }
    pub fn set_reg(&mut self, reg: u8, value: SizedValue){
        use SizedValue::*;
        let r = reg as usize;
        match value{
            SizedValue::None => (), //could potentially throw an error here?
            Byte(v) => {
                if reg & 0x04 == 0{
                    //access lows, AL, CL, DL, BL
                    self.regs[r] = (self.regs[r] & 0xFFFFFF00) | (v as u32);
                }else{
                    //access highs, AH, CH, DH, BH
                    self.regs[r] = (self.regs[r & 0x03] & 0xFFFF00FF) | ((v as u32) << 8);
                }
            },
            Word(v) => {
                self.regs[r] = (self.regs[r] & 0xFFFF0000) | (v as u32);
            },
            Dword(v) => {
                self.regs[r] = v;
            }
        }
    }
    pub fn get_mem(&self, address: u32, size: ValueSize) -> Result<SizedValue, MemoryError>{
        use ValueSize::*;
        match size{
            None => Ok(SizedValue::None),
            Byte => {
                let m = self.memory.get_memory(address)?;
                match m.get(0){
                    Option::None => return Err(MemoryError::Overrun),
                    Some(b) => return Ok(SizedValue::Byte(*b))
                }
            },
            _ => {
                Ok(SizedValue::None)
            }
        }
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
    fn test_memory_ints(){
        let mut m = MemorySystem::default();
        let area = 0x100000;
        let bytes = m.add_memory(area, 0x100, false).unwrap();
        bytes[0x10] = 0x11; 
        bytes[0x11] = 0x22;
        bytes[0x12] = 0x33;
        bytes[0x13] = 0x44;
        assert!(m.get_u8(area + 0x10).unwrap() == 0x11);
        assert!(m.get_u8(area + 0x11).unwrap() == 0x22);
        assert!(m.get_u16(area + 0x10).unwrap() == 0x2211);
        assert!(m.get_u32(area + 0x10).unwrap() == 0x44332211);
        m.set_u32(area + 0x20, 0xAABBCCDD).unwrap();
        m.set_u16(area + 0x30, 0x1234).unwrap();
        let bytes = m.get_memory(area).unwrap(); //reassign bytes to avoid borrowing error, can't overlap with previous set operations
        assert!(bytes[0x20] == 0xDD);
        assert!(bytes[0x21] == 0xCC);
        assert!(bytes[0x22] == 0xBB);
        assert!(bytes[0x23] == 0xAA);
    }
    #[test]
    fn test_memory_failures(){
        let mut m = MemorySystem::default();
        let _bytes = m.add_memory(0x10000, 0x100, false).unwrap();
        assert!(m.add_memory(0x10000, 0x100, false) == Err(MemoryError::ConflictingAdd));
        assert!(m.add_memory(0x100FF, 0x100, false) == Err(MemoryError::UnalignedAdd));
        assert!(m.get_memory(0x10200) == Err(MemoryError::Overrun));
        assert!(m.get_mut_memory(0x10100) == Err(MemoryError::Overrun));
    }

    #[test]
    fn test_register_access(){
        let mut vm = VM::default();
        vm.regs[2] = 0x11223344; // EDX
        vm.regs[4] = 0xFFEEDDBB; // ESP
        assert!(vm.get_reg(2, ValueSize::Dword) == SizedValue::Dword(0x11223344));
        assert!(vm.get_reg(4, ValueSize::Dword) == SizedValue::Dword(0xFFEEDDBB));
        assert!(vm.get_reg(2, ValueSize::Word) == SizedValue::Word(0x3344));
        assert!(vm.get_reg(4, ValueSize::Word) == SizedValue::Word(0xDDBB));
        assert!(vm.get_reg(2, ValueSize::Byte) == SizedValue::Byte(0x44)); //DL
        assert!(vm.get_reg(6, ValueSize::Byte) == SizedValue::Byte(0x33)); //DH
    }
    #[test]
    fn test_register_writes(){
        use SizedValue::*;
        let mut vm = VM::default();
        vm.regs[2] = 0x11223344; // EDX
        vm.regs[4] = 0xFFEEDDBB; // ESP
        vm.set_reg(2, Dword(0xAABBCCDD));
        assert!(vm.get_reg(2, ValueSize::Dword) == SizedValue::Dword(0xAABBCCDD));
        vm.set_reg(4, Word(0x1122));
        assert!(vm.regs[4] == 0xFFEE1122);
        vm.set_reg(2, Byte(0x55)); //DL
        assert!(vm.regs[2] == 0xAABBCC55);
        vm.set_reg(6, Byte(0x66)); //DH
        assert!(vm.regs[6] == 0xAABB6655);
    }
}
