use crate::pipeline::*;
use crate::opcodes::*;
use crate::structs::*;
use crate::memory::*;

#[allow(dead_code)] //remove after design stuff is done

#[derive(Default)]
pub struct VM{
    pub regs: [u32; 8], //EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI
    pub eip: u32,
    pub eflags: u32,

    pub memory: MemorySystem,
    //pub pipeline: Vec<Pipeline>,
    //todo: hypervisor to call external code

    //set when an error has occurred within an opcode execution
    pub errored: Option<OpcodeError>,
    pub error_eip: u32
}

#[derive(PartialEq, Debug)]
#[derive(Copy, Clone)]
pub enum VMError{
    None,
    //memory errors
    ReadBadMemory(u32),
    WroteBadMemory(u32),
    WroteReadOnlyMemory(u32),
    ReadUnloadedMemory(u32),
    //caused by add_memory
    UnalignedMemoryAddition,
    ConflictingMemoryAddition,
    
    //execution error
    InvalidOpcode,

    //decoding error
    DecodingOverrun, //needed more bytes for decoding -- about equivalent to ReadBadMemory
}


impl VM{
    pub fn get_arg(&self, arg: ArgLocation) -> Result<SizedValue, VMError>{
        use ArgLocation::*;
        Ok(match arg{
            None => SizedValue::None,
            Immediate(v) => v,
            Address(a, s) => {
                self.get_mem(a, s)?
            },
            RegisterValue(r, s) => {
                self.get_reg(r, s)
            },
            RegisterAddress(r, s) => {
                self.get_mem(self.get_reg(r, ValueSize::Dword).u32(), s)?
            },
            /*
            ModRMAddress16{offset, reg1, reg2, size} => {
                SizedValue::None
            },*/
            ModRMAddress{offset, reg, size} => {
                SizedValue::None
            },
            SIBAddress{offset, base, scale, index, size} => {
                SizedValue::None
            }
        })
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
    pub fn get_mem(&self, address: u32, size: ValueSize) -> Result<SizedValue, VMError>{
        use ValueSize::*;
        match size{
            None => Ok(SizedValue::None),
            Byte => {
                Ok(SizedValue::Byte(self.memory.get_u8(address)?))
            },
            Word => {
                Ok(SizedValue::Word(self.memory.get_u16(address)?))
            },
            Dword => {
                Ok(SizedValue::Dword(self.memory.get_u32(address)?))
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
        let bytes = m.add_memory(0x10000, 0x100).unwrap();
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
        let bytes = m.add_memory(area, 0x100).unwrap();
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
        let _bytes = m.add_memory(0x10000, 0x100).unwrap();
        assert!(m.add_memory(0x10000, 0x100) == Err(VMError::ConflictingMemoryAddition));
        assert!(m.add_memory(0x100FF, 0x100) == Err(VMError::UnalignedMemoryAddition));
        assert!(m.get_memory(0x10200) == Err(VMError::ReadBadMemory(0x10200)));
        assert!(m.get_mut_memory(0x10100) == Err(VMError::WroteBadMemory(0x10100)));
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

    #[test]
    fn test_get_arg(){
        use SizedValue::*;
        let mut vm = VM::default();
        let area = 0x77660000;
        vm.memory.add_memory(area, 0x100).unwrap();
        vm.memory.set_u32(area + 10, 0x11223344).unwrap();
        vm.regs[0] = area + 12; //eax
        
        let arg = ArgLocation::Immediate(Word(0x9911));
        assert!(vm.get_arg(arg).unwrap() == Word(0x9911));
        let arg = ArgLocation::Address(area + 10, ValueSize::Dword);
        assert!(vm.get_arg(arg).unwrap() == Dword(0x11223344));
        let arg = ArgLocation::RegisterAddress(0, ValueSize::Word);
        assert!(vm.get_arg(arg).unwrap() == Word(0x1122));
        let arg = ArgLocation::RegisterValue(0, ValueSize::Dword);
        assert!(vm.get_arg(arg).unwrap() == Dword(area + 12));
    }
}
