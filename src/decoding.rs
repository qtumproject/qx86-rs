use crate::structs::*;
use crate::opcodes::*;

#[allow(dead_code)] //remove after design stuff is done

fn convert_reg_to_address(reg: u8, size: ValueSize) -> u32{
    0
}

pub fn decode_args(opcode: &Opcode, bytestream: &[u8], args: &mut [OpArgument; MAX_ARGS], address_override: bool) -> Result<usize, usize>{
    use ValueSource::*;
    use std::convert::TryInto;
    let opcode_byte = bytestream[0];
    let mut bytes = &bytestream[0..];
    let mut size = 0;
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
                    let b: [u8; 2] = match &bytes.try_into(){
                    Ok(res) => *res,
                    Err(_) => return Err(size)
                };
                args[0].location = ValueLocation::Address(u16::from_le_bytes(b) as u32);
                args[0].size = opcode.arg_size[0];
                2
            } else {
                bytes = &bytes[1..]; //advance by one
                let b: [u8; 4] = match &bytes.try_into(){
                    Ok(res) => *res,
                    Err(_) => return Err(size)
                };
                args[0].location = ValueLocation::Address(u32::from_le_bytes(b));
                args[0].size = opcode.arg_size[0];
                4
            }
        }
        ImmediateValue => {
            2
        },
        RegisterSuffix =>{
            args[0].location = ValueLocation::Address((opcode_byte & 0x7) as u32);
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
    #[test]
    fn decode_immediate_address(){
        let mut args:[OpArgument; MAX_ARGS] = Default::default();
        let mut opcode:Opcode = Default::default();
        opcode.arg_source[0] = ValueSource::ImmediateAddress;
        let bytes:&[u8] = &[
            0xFA, //the opcode
            0x11, //argument begin
            0x22,
            0x33,
            0x44]; //argument end
        
        match decode_args(&opcode, bytes, &mut args, false){
            Err(_) => assert!(false, "decode resulted in error"),
            Ok(s) => assert_eq!(s, 4)
        };
        assert!(args[0].location == ValueLocation::Address(0x44332211));
    }
}