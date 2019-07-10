use crate::structs::*;
use crate::opcodes::*;
use crate::vm::*;

#[allow(dead_code)] //remove after design stuff is done


fn u32_from_bytes(bytes: &[u8]) -> Result<u32, VMError>{
    use std::convert::TryInto;
    if bytes.len() < 4 {
        return Err(VMError::DecodingOverrun);
    }
    let b: [u8; 4] = *(&bytes[0..4].try_into().unwrap());
    Ok(u32::from_le_bytes(b))
}

fn u16_from_bytes(bytes: &[u8]) -> Result<u16, VMError>{
    use std::convert::TryInto;
    if bytes.len() < 2 {
        return Err(VMError::DecodingOverrun);
    }
    let b: [u8; 2] = *(&bytes[0..2].try_into().unwrap());
    Ok(u16::from_le_bytes(b))
}
fn u8_from_bytes(bytes: &[u8]) -> Result<u8, VMError>{
    use std::convert::TryInto;
    if bytes.len() < 1 {
        return Err(VMError::DecodingOverrun);
    }
    Ok(bytes[0])
}

#[derive(Default)]
struct ModRM{
    rm: u8, //3 bits
    reg: u8, //3 bits
    mode: u8, //2 bits
}

impl ModRM{
    fn parse(b: u8) -> ModRM{
        ModRM{
            rm: b & 0x07, //bottom 3 bits
            reg: (b & (0x07 << 3)) >> 3, //middle 3 bits
            mode: (b & (0x03 << 6)) >> 6 //top 2 bits
        }
    }
    //This is pretty dense because Mod R/M is stupidly complicated
    //Make sure to use this reference to understand why: http://ref.x86asm.net/coder32.html#modrm_byte_32
    fn decode(&self, sib: &SIB, disp: i32, size: ValueSize) -> ArgLocation{
        //when mode is 3, actual uses the direct register, and thus will not be an address
        if self.mode == 3 {
            return ArgLocation::RegisterValue(self.rm, size);
        }
        //special case for [disp32]
        if self.mode == 0 && self.rm == 5 {
            return ArgLocation::Address(disp as u32, size);
        }
        
        //exclude rm == 4 as that is SIB option
        //no disp, just register address
        if self.mode == 0 && self.rm != 4 {
            return ArgLocation::RegisterAddress(self.rm, size)
        }
        //[reg32 + disp] (where disp can be 8 or 32 bit)
        if (self.mode == 2 || self.mode == 3) && self.rm != 4{
            return ArgLocation::ModRMAddress{
                offset: Some(disp),
                reg: Some(self.rm),
                size: size
            };
        }

        //Only remaining options now is 
        //[SIB], [SIB+disp8], [SIB+disp32]

        let base = if sib.base == 5 {
            //either disp32, ebp+disp8, or ebp+disp32 for modrm.mode values 0, 1, 2 respectively
            if self.mode == 0{
                Option::None //no register base
            }else{
                Some(Reg32::EBP as u8)
            }
        }else{
            Some(sib.base)
        };
        //No index if index is 4, otherwise index corresponds to register
        let index = if sib.index == 4{
            Option::None
        }else{
            Some(sib.index)
        };

        //effective address form: [offset + base + (scale * index)]
        return ArgLocation::SIBAddress{
            offset: disp, //is 0 when not actually used or specified, thus not affecting the effective address calculation
            base: base, //optional register
            scale: 1 << sib.scale, //Equates to 1, 2, 4, 8 from values 0, 1, 2, 3 respectively
            index: index, //optional register]
            size: size
        };
    }
}

#[derive(Default)]
struct SIB{
    base: u8, //3 bits
    index: u8, //3 bits
    scale: u8 //2 bits
}

impl SIB{
    fn parse(b: u8) -> SIB{
        SIB{
            base: b & 0x07, //bottom 3 bits
            index: (b & (0x07 << 3)) >> 3, //middle 3 bits
            scale: (b & (0x03 << 6)) >> 6, //top 2 bits
        }
    }
}

pub fn decode_args(opcode: &Opcode, bytestream: &[u8], args: &mut [OpArgument; MAX_ARGS], address_override: bool) -> Result<usize, VMError>{
    use ArgSource::*;
    if bytestream.len() < 16{
        return Err(VMError::DecodingOverrun);
    }
    let opcode_byte = bytestream[0];
    let mut bytes = &bytestream[1..];
    let mut size:usize = 1; //to count for opcode
    let mut modrm = self::ModRM::default();
    let mut sib = SIB::default();
    //note displacements are treated as signed numbers
    let mut modrm_disp:i32 = 0;

    if opcode.has_modrm{
        modrm = self::ModRM::parse(bytes[0]);
        bytes = &bytes[1..]; //advance to next byte
        size += 1;
        if modrm.mode != 3 && (modrm.rm == 4){
            sib = SIB::parse(bytes[0]);
            bytes = &bytes[1..];
            size += 1;
        }
        //read in immediate displacement
        //first do 32 bit displacements
        if  (modrm.mode == 0 && modrm.rm == 5) ||
            (modrm.mode == 2) ||
            (modrm.mode == 0 && sib.base == 5) {
            
            modrm_disp = u32_from_bytes(bytes)? as i32;
            bytes = &bytes[4..];
            size += 4;
        } else if modrm.mode == 1 {
            modrm_disp = u8_from_bytes(bytes)? as i32;
            bytes = &bytes[1..];
            size += 1;
        }
    }
    //todo: parse modr/m byte here if present, before actually parsing arguments
    for n in 0..3{
        let advance = match opcode.arg_source[n] {
            None => {
                0
            },
            ModRM => {
                args[n].location = modrm.decode(&sib, modrm_disp, opcode.arg_size[n]);
                0 //size calculation was done before here, so don't need to advance any
            },
            ModRMReg => {
                args[n].location = ArgLocation::RegisterAddress(modrm.reg, opcode.arg_size[n]);
                0
            },
            ImmediateAddress =>{
                args[n].location = ArgLocation::Address(u32_from_bytes(bytes)?, opcode.arg_size[n]);
                4
            }
            ImmediateValue | JumpRel => {
                let (loc, sz) = match opcode.arg_size[n]{
                    ValueSize::None => (ArgLocation::Immediate(SizedValue::None), 0),
                    ValueSize::Byte => (ArgLocation::Immediate(SizedValue::Byte(bytes[0])), 1),
                    ValueSize::Word => {
                        (ArgLocation::Immediate(SizedValue::Word(u16_from_bytes(bytes)?)), 2)
                    },
                    ValueSize::Dword => {
                        (ArgLocation::Immediate(SizedValue::Dword(u32_from_bytes(bytes)?)), 4)
                    }
                };
                args[n].location = loc;
                sz
            },
            RegisterSuffix =>{
                args[n].location = ArgLocation::RegisterValue(opcode_byte & 0x7, opcode.arg_size[n]);
                0
            }
        };
        bytes = &bytes[(advance as usize)..];
        size += advance as usize;
    }
    Ok(size)
}



#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    //helper function to simplify testing
    fn decode_arg(source: ArgSource, size: ValueSize, bytecode: &[u8]) -> (OpArgument, usize){
        let mut args:[OpArgument; MAX_ARGS] = Default::default();
        let mut opcode:Opcode = Default::default();
        opcode.arg_source[0] = source;
        opcode.arg_size[0] = size;
        
        let res = match decode_args(&opcode, bytecode, &mut args, false){
            Err(_) => {
                assert!(false, "decode resulted in error");
                0
            },
            Ok(s) => s
        };
        (args[0], res) 
    }
    #[test]
    fn decode_immediate_address(){
        let mut bytes = vec![
            0xFA, //the opcode
            0x11, //argument begin
            0x22,
            0x33,
            0x44 //argument end
        ];
        bytes.resize(bytes.len() + 16, 0);
        let (arg, size) = decode_arg(ArgSource::ImmediateAddress, ValueSize::Byte, &bytes);

        assert!(arg.location == ArgLocation::Address(0x44332211, ValueSize::Byte));
        assert!(size == 5);
    }
    #[test]
    fn decode_immediate_value(){
        let mut bytes = vec![
            0xFA, //the opcode
            0x11, //argument begin
            0x22,
            0x33,
            0x44, //argument end
            0x88
        ];
        bytes.resize(bytes.len() + 16, 0);
        {
            let (arg, size) = decode_arg(ArgSource::ImmediateValue, ValueSize::Byte, &bytes);
            assert!(arg.location == ArgLocation::Immediate(SizedValue::Byte(0x11)));
            assert!(size == 2);
        }
        {
            let (arg, size) = decode_arg(ArgSource::ImmediateValue, ValueSize::Word, &bytes);
            assert!(arg.location == ArgLocation::Immediate(SizedValue::Word(0x2211)));
            assert!(size == 3);
        }
        {
            let (arg, size) = decode_arg(ArgSource::ImmediateValue, ValueSize::Dword, &bytes);
            assert!(arg.location == ArgLocation::Immediate(SizedValue::Dword(0x44332211)));
            assert!(size == 5);
        }
    }
    #[test]
    fn decode_register_suffix_value(){
        let mut bytes = vec![
            0xF3
        ];
        bytes.resize(bytes.len() + 16, 0);
        let (arg, size) = decode_arg(ArgSource::RegisterSuffix, ValueSize::Dword, &bytes);
        assert!(size == 1);
        assert!(arg.location == ArgLocation::RegisterValue(3, ValueSize::Dword));
    }
}