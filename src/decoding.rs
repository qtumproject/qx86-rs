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
    if bytes.len() < 1 {
        return Err(VMError::DecodingOverrun);
    }
    Ok(bytes[0])
}

#[derive(Default)]
#[derive(Debug)]
#[derive(Clone, Copy)]
pub struct ModRM{
    rm: u8, //3 bits
    pub reg: u8, //3 bits
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
    fn decode(&self, sib: &SIB, disp: u32, size: ValueSize) -> ArgLocation{
        //when mode is 3, actual uses the direct register, and thus will not be an address
        if self.mode == 3 {
            return ArgLocation::RegisterValue(self.rm, size);
        }
        //special case for [disp32]
        if self.mode == 0 && self.rm == 5 {
            return ArgLocation::Address(disp, size);
        }
        
        //exclude rm == 4 as that is SIB option
        //no disp, just register address
        if self.mode == 0 && self.rm != 4 {
            return ArgLocation::RegisterAddress(self.rm, size)
        }
        //[reg32 + disp] (where disp can be 8 or 32 bit)
        if (self.mode == 1 || self.mode == 2) && self.rm != 4{
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
#[derive(Clone, Copy)]
pub struct SIB{
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

#[derive(Default)]
#[derive(Clone, Copy)]
pub struct ParsedModRM{
    pub modrm: ModRM,
    pub sib: Option<SIB>,
    pub disp: Option<u32>,
    pub size: u8
}

impl ParsedModRM{
    pub fn from_bytes(bytestream: &[u8]) -> Result<ParsedModRM, VMError>{
        if bytestream.len() < 16{
            return Err(VMError::DecodingOverrun);
        }
        let mut parsed = ParsedModRM::default();
        let mut bytes = &bytestream[1..]; //skip opcode
        let mut size = 0;
        parsed.modrm = self::ModRM::parse(bytes[0]);
        bytes = &bytes[1..]; //advance to next byte
        size += 1;
        if parsed.modrm.mode != 3 && parsed.modrm.rm == 4 {
            parsed.sib = Some(SIB::parse(bytes[0]));
            bytes = &bytes[1..];
            size += 1;
        }
        //read in immediate displacement
        //first do 32 bit displacements
        if  (parsed.modrm.mode == 0 && parsed.modrm.rm == 5) ||
            (parsed.modrm.mode == 2) ||
            (parsed.modrm.mode == 0 && parsed.sib.unwrap_or_default().base == 5) {
            
            parsed.disp = Some(u32_from_bytes(bytes)?);
            size += 4;
        } else if parsed.modrm.mode == 1 {
            parsed.disp = Some(((u8_from_bytes(bytes)? as i8) as i32) as u32);
            size += 1;
        }
        parsed.size = size;
        Ok(parsed)
    }
}

pub fn decode_args(opcode: &Opcode, bytestream: &[u8], args: &mut [OpArgument; MAX_ARGS], address_override: bool) -> Result<usize, VMError>{
    decode_args_with_modrm(opcode, bytestream, args, address_override, None)
}
pub fn decode_args_with_modrm(opcode: &Opcode, bytestream: &[u8], args: &mut [OpArgument; MAX_ARGS], _address_override: bool, parsed_modrm: Option<ParsedModRM>) -> Result<usize, VMError>{
    use ArgSource::*;
    if bytestream.len() < 16{
        return Err(VMError::DecodingOverrun);
    }
    let opcode_byte = bytestream[0];
    let mut bytes = &bytestream[1..];
    let mut size:usize = 1; //to count for opcode
    let modrm = parsed_modrm.unwrap_or_default();
    size += modrm.size as usize;
    bytes = &bytes[modrm.size as usize..];
    //note displacements are treated as signed numbers
    //todo: parse modr/m byte here if present, before actually parsing arguments
    for n in 0..3{
        let advance = match opcode.arg_source[n] {
            None => {
                0
            },
            ModRM => {
                args[n].location = modrm.modrm.decode(&modrm.sib.unwrap_or_default(), modrm.disp.unwrap_or(0), opcode.arg_size[n]);
                0 //size calculation was done before here, so don't need to advance any
            },
            ModRMReg => {
                args[n].location = ArgLocation::RegisterValue(modrm.modrm.reg, opcode.arg_size[n]);
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
            Literal(l) => {
                args[n].location = ArgLocation::Immediate(l);
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
    //helper function to simplify testing
    fn decode_arg_modrm(source: ArgSource, size: ValueSize, bytecode: &[u8]) -> (OpArgument, usize){
        let mut args:[OpArgument; MAX_ARGS] = Default::default();
        let mut opcode:Opcode = Default::default();
        opcode.arg_source[0] = source;
        opcode.arg_size[0] = size;

        let modrm = ParsedModRM::from_bytes(bytecode).unwrap();
        
        let res = match decode_args_with_modrm(&opcode, bytecode, &mut args, false, Some(modrm)){
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

    #[test]
    fn decode_modrm(){
        {
            let mut bytes = vec![
                0xFA, //opcode
                0x0B //mod rm [ebx] with /r=ecx/cx
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Word, &bytes);
            assert_eq!(size, 2);
            assert_eq!(arg.location, ArgLocation::RegisterAddress(Reg32::EBX as u8, ValueSize::Word));
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Word, &bytes);
            assert_eq!(size, 2);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg16::CX as u8, ValueSize::Word));
        }
        {
            let mut bytes = vec![
                0xFA, //opcode
                0x25,//mod rm [disp32] with /r=esp/ah
                0x11, 0x22, 0x33, 0x44 //disp32
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Byte, &bytes);
            assert_eq!(size, 6);
            assert_eq!(arg.location, ArgLocation::Address(0x44332211, ValueSize::Byte));
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Byte, &bytes);
            assert_eq!(size, 6);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg8::AH as u8, ValueSize::Byte));
        }
        {
            let mut bytes = vec![
                0xFA, //opcode
                0x47,//mod rm [EDI + disp8] with /r=eax
                0x12 //disp8
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Dword, &bytes);
            assert_eq!(size, 3);
            assert_eq!(arg.location, ArgLocation::ModRMAddress{
                offset: Some(0x12),
                reg: Some(Reg32::EDI as u8),
                size: ValueSize::Dword
            });
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Dword, &bytes);
            assert_eq!(size, 3);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg32::EAX as u8, ValueSize::Dword));
        }
        {
            let mut bytes = vec![
                0xFA, //opcode
                0xDD,//mod rm ebp with /r=ebx
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Word, &bytes);
            assert_eq!(size, 2);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg16::BP as u8, ValueSize::Word));
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Word, &bytes);
            assert_eq!(size, 2);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg16::BX as u8, ValueSize::Word));
        }
    }
    #[test]
    fn decode_sib(){
        {
            let mut bytes = vec![
                0xFA, //opcode
                0x0C,//modrm [sib] with /r=ecx
                0xAF, //[EBP*4 + EDI]
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Dword, &bytes);
            assert_eq!(size, 3);
            assert_eq!(arg.location, ArgLocation::SIBAddress{
                offset: 0,
                base: Some(Reg32::EDI as u8),
                scale: 4,
                index: Some(Reg32::EBP as u8),
                size: ValueSize::Dword
            });
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Dword, &bytes);
            assert_eq!(size, 3);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg32::ECX as u8, ValueSize::Dword));
        }
        {
            let mut bytes = vec![
                0xFA, //opcode
                0x0C,//modrm [sib] with /r=ecx
                0x61, //[(none) + ECX]
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Dword, &bytes);
            assert_eq!(size, 3);
            assert_eq!(arg.location, ArgLocation::SIBAddress{
                offset: 0,
                base: Some(Reg32::ECX as u8),
                scale: 2, //this is not actually used, but is set
                index: None,
                size: ValueSize::Dword
            });
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Dword, &bytes);
            assert_eq!(size, 3);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg32::ECX as u8, ValueSize::Dword));
        }
        {
            let mut bytes = vec![
                0xFA, //opcode
                0x0C,//modrm [sib] with /r=ecx
                0xCD, //[(none + disp32) * 8 + ECX]
                0x11, 0x22, 0x33, 0x44 //disp32
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Dword, &bytes);
            assert_eq!(size, 7);
            assert_eq!(arg.location, ArgLocation::SIBAddress{
                offset: 0x44332211,
                base: None,
                scale: 8, 
                index: Some(Reg32::ECX as u8),
                size: ValueSize::Dword
            });
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Dword, &bytes);
            assert_eq!(size, 7);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg32::ECX as u8, ValueSize::Dword));
        }
        {
            let mut bytes = vec![
                0xFA, //opcode
                0x44,//modrm [sib + imm8] with /r=eax
                0x8D, //[(ecx * 4) + ebp] (+imm8 from modrm)
                0x11 //disp8
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Dword, &bytes);
            assert_eq!(size, 4);
            assert_eq!(arg.location, ArgLocation::SIBAddress{
                offset: 0x11,
                base: Some(Reg32::EBP as u8),
                scale: 4, 
                index: Some(Reg32::ECX as u8),
                size: ValueSize::Dword
            });
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Dword, &bytes);
            assert_eq!(size, 4);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg32::EAX as u8, ValueSize::Dword));
        }
        {
            let mut bytes = vec![
                0xFA, //opcode
                0x44,//modrm [sib + imm8] with /r=eax
                0x8D, //[(ecx * 4) + ebp] (+imm8 from modrm)
                0xEF //disp8 (-0x11)
            ];
            bytes.resize(bytes.len() + 16, 0);
            let (arg, size) = decode_arg_modrm(ArgSource::ModRM, ValueSize::Dword, &bytes);
            assert_eq!(size, 4);
            assert_eq!(arg.location, ArgLocation::SIBAddress{
                offset: (-0x11i32) as u32, //should be -0x11 in an i32
                base: Some(Reg32::EBP as u8),
                scale: 4, 
                index: Some(Reg32::ECX as u8),
                size: ValueSize::Dword
            });
            let (arg, size) = decode_arg_modrm(ArgSource::ModRMReg, ValueSize::Dword, &bytes);
            assert_eq!(size, 4);
            assert_eq!(arg.location, ArgLocation::RegisterValue(Reg32::EAX as u8, ValueSize::Dword));
        }
    }
}