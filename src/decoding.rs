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

pub fn decode_args(opcode: &Opcode, bytestream: &[u8], args: &mut [OpArgument; MAX_ARGS], address_override: bool) -> Result<usize, VMError>{
    use ArgSource::*;
    let opcode_byte = bytestream[0];
    let mut bytes = &bytestream[1..];
    let mut size:usize = 1; //to count for opcode
    //todo: parse modr/m byte here if present, before actually parsing arguments
    for n in 0..3{
        let advance = match opcode.arg_source[n] {
            None => {
                0
            },
            ModRM => {
                1
            },
            ModRMReg => {
                1
            },
            ImmediateAddress =>{
                args[n].location = ArgLocation::Address(u32_from_bytes(bytes)?, opcode.arg_size[n]);
                args[n].size = 4;
                4
            }
            ImmediateValue => {
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
            },
            JumpRel8 =>{
                1
            },
            JumpRel16 => {
                2
            },
            JumpRel32 => {
                4
            }
        };
        args[n].size = advance;
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
        let bytes:&[u8] = &[
            0xFA, //the opcode
            0x11, //argument begin
            0x22,
            0x33,
            0x44 //argument end
        ];
        let (arg, size) = decode_arg(ArgSource::ImmediateAddress, ValueSize::Byte, bytes);

        assert!(arg.location == ArgLocation::Address(0x44332211, ValueSize::Byte));
        assert!(size == 5);
    }
    #[test]
    fn decode_immediate_value(){
        let bytes = [
            0xFA, //the opcode
            0x11, //argument begin
            0x22,
            0x33,
            0x44, //argument end
            0x88
        ];
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
        let bytes = [
            0xF3
        ];
        let (arg, size) = decode_arg(ArgSource::RegisterSuffix, ValueSize::Dword, &bytes);
        assert!(size == 1);
        assert!(arg.location == ArgLocation::RegisterValue(3, ValueSize::Dword));
    }
}