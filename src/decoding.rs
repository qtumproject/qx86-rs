use crate::structs::*;
use crate::opcodes::*;

#[allow(dead_code)] //remove after design stuff is done

fn convert_reg_to_address(reg: u8, size: ValueSize) -> u32{
    const REG_FLAG:u32 = 1 << 30;
    const DWORD_BEGIN:u32 = 0;
    const WORD_BEGIN:u32 = 8;
    const BYTE_BEGIN:u32 = 12;
    const NONE_BEGIN:u32 = 32; //anything greater than this will equate to 0
    let begin = match size{
        ValueSize::Byte => BYTE_BEGIN,
        ValueSize::Word => WORD_BEGIN,
        ValueSize::Dword => DWORD_BEGIN,
        ValueSize::None => NONE_BEGIN
    };

    //in theory should maybe panic or something if reg > 7? 
    REG_FLAG | ((reg & 0x07) as u32) + begin
}


pub enum DecodeError{
    MemoryError
}



fn u32_from_bytes(bytes: &[u8]) -> Result<u32, DecodeError>{
    use std::convert::TryInto;
    if bytes.len() < 4 {
        return Err(DecodeError::MemoryError);
    }
    let b: [u8; 4] = *(&bytes[0..4].try_into().unwrap());
    Ok(u32::from_le_bytes(b))
}

fn u16_from_bytes(bytes: &[u8]) -> Result<u16, DecodeError>{
    use std::convert::TryInto;
    if bytes.len() < 2 {
        return Err(DecodeError::MemoryError);
    }
    let b: [u8; 2] = *(&bytes[0..2].try_into().unwrap());
    Ok(u16::from_le_bytes(b))
}

pub fn decode_args(opcode: &Opcode, bytestream: &[u8], args: &mut [OpArgument; MAX_ARGS], address_override: bool) -> Result<usize, DecodeError>{
    use ValueSource::*;
    let opcode_byte = bytestream[0];
    let mut bytes = &bytestream[0..];
    let mut size:usize = 0;
    size += match opcode.arg_source[0] {
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
            if address_override{
                bytes = &bytes[1..]; //advance by one
                args[0].location = ValueLocation::Address(u16_from_bytes(bytes)? as u32);
                args[0].size = 2;
                2
            } else {
                bytes = &bytes[1..]; //advance by one
                args[0].location = ValueLocation::Address(u32_from_bytes(bytes)?);
                args[0].size = 4;
                4
            }
        }
        ImmediateValue => {
            bytes = &bytes[1..]; //advance by one
            let (loc, sz) = match opcode.arg_size[0]{
                ValueSize::None => (ValueLocation::Immediate(SizedValue::None), 0),
                ValueSize::Byte => (ValueLocation::Immediate(SizedValue::Byte(bytes[0])), 1),
                ValueSize::Word => {
                    (ValueLocation::Immediate(SizedValue::Word(u16_from_bytes(bytes)?)), 2)
                },
                ValueSize::Dword => {
                    (ValueLocation::Immediate(SizedValue::Dword(u32_from_bytes(bytes)?)), 4)
                }
            };
            args[0].location = loc;
            args[0].size = sz;
            args[0].size as usize
        },
        RegisterSuffix =>{
            args[0].location = ValueLocation::Address(convert_reg_to_address(opcode_byte & 0x7, opcode.arg_size[0]));
            0
        }
    };
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
    fn decode_arg(source: ValueSource, size: ValueSize, bytecode: &[u8]) -> (OpArgument, usize){
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
        let (arg, size) = decode_arg(ValueSource::ImmediateAddress, ValueSize::Byte, bytes);

        assert!(arg.location == ValueLocation::Address(0x44332211));
        assert!(size == 4);
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
            let (arg, size) = decode_arg(ValueSource::ImmediateValue, ValueSize::Byte, &bytes);
            assert!(arg.location == ValueLocation::Immediate(SizedValue::Byte(0x11)));
            assert!(size == 1);
        }
        {
            let (arg, size) = decode_arg(ValueSource::ImmediateValue, ValueSize::Word, &bytes);
            assert!(arg.location == ValueLocation::Immediate(SizedValue::Word(0x2211)));
            assert!(size == 2);
        }
        {
            let (arg, size) = decode_arg(ValueSource::ImmediateValue, ValueSize::Dword, &bytes);
            assert!(arg.location == ValueLocation::Immediate(SizedValue::Dword(0x44332211)));
            assert!(size == 4);
        }
    }
}