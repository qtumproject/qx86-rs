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

    //set to indicate diagnostic info when an error occurs
    pub error_eip: u32
}

#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum Reg32{
    EAX = 0,
    ECX,
    EDX,
    EBX,
    ESP,
    EBP,
    ESI,
    EDI
}

#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum Reg16{
    AX = 0,
    CX,
    DX,
    BX,
    SP,
    BP,
    SI,
    DI
}

#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum Reg8{
    AL = 0,
    CL,
    DL,
    BL,
    AH,
    CH,
    DH,
    BH
}

#[derive(PartialEq, Debug)]
#[derive(Copy, Clone)]
pub enum VMError{
    None,
    NotYetImplemented, //ideally throw none of these when finished
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
    WroteUnwriteableArgumnet,

    //decoding error
    DecodingOverrun, //needed more bytes for decoding -- about equivalent to ReadBadMemory
    
    //argument errors
    WrongSizeExpectation,
    TooBigSizeExpectation,

    InternalVMStop, //not an actual error but triggers a stop
}


impl VM{
    fn calculate_modrm_address(&self, arg: &ArgLocation) -> u32{
        use ArgLocation::*;
        match arg{
            ModRMAddress{offset, reg, size: _} => {
                let o = match offset{
                    Some(x) => *x,
                    Option::None => 0
                };
                let r = match reg{
                    Some(x) => self.regs[*x as usize],
                    Option::None => 0
                };
                o.wrapping_add(r)
            },
            SIBAddress{offset, base, scale, index, size: _} => {
                let b = match base{
                    Some(x) => self.regs[*x as usize],
                    Option::None => 0
                };
                let ind = match index{
                    Some(x) => self.regs[*x as usize],
                    Option::None => 0
                };
                //base + (index * scale) + offset
                let address = b.wrapping_add(ind.wrapping_mul(*scale as u32)).wrapping_add(*offset);
                address
            },
            _ => panic!("This should not be reached")
        }
    }
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
                self.get_mem(self.get_reg(r, ValueSize::Dword).u32_exact()?, s)?
            },
            /*
            ModRMAddress16{offset, reg1, reg2, size} => {
                SizedValue::None
            },*/
            ModRMAddress{offset: _, reg: _, size} => {
                self.get_mem(self.calculate_modrm_address(&arg), size)?
            },
            SIBAddress{offset: _, base: _, scale: _, index: _, size} => {
                self.get_mem(self.calculate_modrm_address(&arg), size)?
            }
        })
    }
    //When the specified ArgLocation does not match the size of SizedValue, the SizedValue will be zero extended to fit
    //If the SizedValue is larger than can fit, then an error will be returned
    pub fn set_arg(&mut self, arg: ArgLocation, v: SizedValue) -> Result<(), VMError>{
        use ArgLocation::*;
        match arg{
            None => (),
            Immediate(_v) => return Err(VMError::WroteUnwriteableArgumnet), //should never happen, this is an implementation error
            Address(a, s) => {
                self.set_mem(a, v.convert_size_zx(s)?)?
            },
            RegisterValue(r, s) => {
                self.set_reg(r, v.convert_size_zx(s)?)
            },
            RegisterAddress(r, s) => {
                self.set_mem(self.get_reg(r, ValueSize::Dword).u32_exact()?, v.convert_size_zx(s)?)?
            },
            ModRMAddress{offset: _, reg: _, size} => {
                let sized = v.convert_size_trunc(size);
                self.set_mem(self.calculate_modrm_address(&arg), sized)?
            },
            SIBAddress{offset: _, base: _, scale: _, index: _, size} => {
                let sized = v.convert_size_trunc(size);
                self.set_mem(self.calculate_modrm_address(&arg), sized)?
            }
        };

        Ok(())
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
                    self.regs[r & 0x03] = (self.regs[r & 0x03] & 0xFFFF00FF) | ((v as u32) << 8);
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
    pub fn set_mem(&mut self, address: u32, value: SizedValue) -> Result<(), VMError>{
        use SizedValue::*;
        if address & 0x80000000 == 0{
            //in read-only memory
            return Err(VMError::WroteReadOnlyMemory(address));
        }
        match value{
            None => (),
            Byte(v) => {
                self.memory.set_u8(address, v)?;
            },
            Word(v) => {
                self.memory.set_u16(address, v)?;
            },
            Dword(v) => {
                self.memory.set_u32(address, v)?;
            }
        };
        Ok(())
    }


    //note: errors.len() must be equal to pipeline.len() !! 
    fn cycle(&mut self, pipeline: &mut [Pipeline], errors: &mut [Result<(), VMError>]) -> Result<bool, VMError>{
        fill_pipeline(self, &OPCODES[0..], pipeline)?;
        let mut error_eip = self.eip;
        //manually unroll loop later if needed?
        for n in 0..pipeline.len() {
            let p = &pipeline[n];
            errors[n] = (p.function)(self, p);
            self.eip += p.eip_size as u32;
        }
        //check for errors
        for n in 0..pipeline.len(){
            if errors[n].is_err(){
                if errors[n].err().unwrap() == VMError::InternalVMStop{
                    //This is to set eip to the point of the stop, rather than the opcode after
                    self.eip = error_eip;
                    return Ok(true);
                } else {
                    self.error_eip = error_eip; 
                    return Err(errors[n].err().unwrap());
                }
            }
            error_eip += pipeline[n].eip_size as u32;
        }
        Ok(false)
    }
    pub fn execute(&mut self) -> Result<bool, VMError>{
        //todo: gas handling
        let mut pipeline = vec![];
        pipeline.resize(PIPELINE_SIZE, Pipeline::default());
        let mut errors = vec![];
        errors.resize(PIPELINE_SIZE, Result::Ok(()));
        loop{
            if self.cycle(&mut pipeline, &mut errors)? {
                return Ok(true);
            }
        }
    }
    pub fn copy_into_memory(&mut self, address: u32, data: &[u8]) -> Result<(), VMError>{
        let m = self.memory.get_mut_sized_memory(address, data.len() as u32)?;
        m[0..data.len()].copy_from_slice(data);
        Ok(())
    }
    pub fn reg8(&self, r: Reg8) -> u8{
        self.get_reg(r as u8, ValueSize::Byte).u8_exact().unwrap()
    }
    pub fn reg16(&self, r: Reg16) -> u16{
        self.get_reg(r as u8, ValueSize::Word).u16_exact().unwrap()
    }
    pub fn reg32(&self, r: Reg32) -> u32{
        self.get_reg(r as u8, ValueSize::Dword).u32_exact().unwrap()
    }
    pub fn set_reg8(&mut self, r: Reg8, v: u8){
        self.set_reg(r as u8, SizedValue::Byte(v));
    }
    pub fn set_reg16(&mut self, r: Reg8, v: u16){
        self.set_reg(r as u8, SizedValue::Word(v));
    }
    pub fn set_reg32(&mut self, r: Reg8, v: u32){
        self.set_reg(r as u8, SizedValue::Dword(v));
    }
}

const PIPELINE_SIZE:usize = 16;

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
        assert_eq!(vm.regs[2], 0xAABB6655);
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

    #[test]
    fn test_set_arg(){
        use SizedValue::*;
        let mut vm = VM::default();
        let area = 0x87660000; //make sure top bit of memory area is set so it's writeable
        vm.memory.add_memory(area, 0x100).unwrap();
        vm.memory.set_u32(area + 10, 0x11223344).unwrap();
        vm.regs[0] = area + 12; //eax
        

        let arg = ArgLocation::Immediate(Word(0x9911));
        assert!(vm.set_arg(arg, SizedValue::Byte(0x11)).is_err());

        let arg = ArgLocation::Address(area + 10, ValueSize::Dword);
        vm.set_arg(arg, SizedValue::Dword(0xAABBCCDD)).unwrap();
        assert!(vm.memory.get_u32(area + 10).unwrap() == 0xAABBCCDD);

        let arg = ArgLocation::RegisterAddress(0, ValueSize::Word);
        vm.set_arg(arg, SizedValue::Word(0x1122)).unwrap();
        assert!(vm.memory.get_u32(area + 10).unwrap() == 0x1122CCDD);
        //test that it fails for too large of values
        assert!(vm.set_arg(arg, SizedValue::Dword(0xABCDEF01)).is_err());
        //and that memory is unmodified afterwards
        assert!(vm.memory.get_u32(area + 10).unwrap() == 0x1122CCDD);

        vm.set_arg(arg, SizedValue::Byte(0x11)).unwrap();
        assert!(vm.memory.get_u32(area + 10).unwrap() == 0x0011CCDD);

        let arg = ArgLocation::RegisterValue(1, ValueSize::Dword);
        vm.set_arg(arg, SizedValue::Dword(0x99887766)).unwrap();
        assert!(vm.regs[1] == 0x99887766);
    }
    #[test]
    fn test_set_arg_readonly(){
        let mut vm = VM::default();
        let area = 0x07660000;
        vm.memory.add_memory(area, 0x100).unwrap();
        vm.memory.set_u32(area + 10, 0x11223344).unwrap();
        vm.regs[0] = area + 12; //eax
        
        let arg = ArgLocation::Address(area + 10, ValueSize::Dword);
        //ensure it errors
        assert!(vm.set_arg(arg, SizedValue::Dword(0xAABBCCDD)).is_err());
        //and that memory is unchanged
        assert!(vm.memory.get_u32(area + 10).unwrap() == 0x11223344);
    }
    #[test]
    fn test_modrm_address_calculation(){
        let mut vm = VM::default();
        let a = ArgLocation::ModRMAddress{
            offset: None,
            reg: Some(Reg32::EBX as u8),
            size: ValueSize::Byte
        };
        vm.regs[Reg32::EBX as usize] = 0x88112233;
        assert_eq!(vm.calculate_modrm_address(&a), 0x88112233);
        let a = ArgLocation::ModRMAddress{
            offset: Some(0x11223344),
            reg: Some(Reg32::EBX as u8),
            size: ValueSize::Byte
        };
        assert_eq!(vm.calculate_modrm_address(&a), 0x99335577);
        let a = ArgLocation::ModRMAddress{
            offset: Some(0x11223344),
            reg: None,
            size: ValueSize::Byte
        };
        assert_eq!(vm.calculate_modrm_address(&a), 0x11223344);
    }
    #[test]
    fn test_sib_address_calculation(){
        let mut vm = VM::default();
        let a = ArgLocation::SIBAddress{
            offset: 1,
            base: Some(Reg32::EBX as u8),
            scale: 2,
            index: Some(Reg32::EDI as u8),
            size: ValueSize::Byte
        };
        vm.regs[Reg32::EBX as usize] = 0x11223344;
        vm.regs[Reg32::EDI as usize] = 0xFFEEDDCC;
        //(ffeeddcc * 2) + 11223344 + 1
        //(ffddbb98) + 11223344 + 1
        //10ffeddc + 1
        //10ffeddd
        assert_eq!(vm.calculate_modrm_address(&a), 0x10FFEEDD);

    }
}
