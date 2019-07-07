#[macro_use]
extern crate lazy_static;

#[allow(dead_code)] //remove after design stuff is done
pub mod qx86{

    const MAX_ARGS:usize = 3;

    #[derive(Copy, Clone)]
    enum ValueSize{
        None,
        Byte,
        Word,
        DWord
    }

    #[derive(Copy, Clone)]
    enum ValueSource{
        None,
        ModRM,
        ModRMReg, //the /r field
        ImmediateAddress,
        ImmediateValue,
        RegisterSuffix //lowest 3 bits of the opcode is used for register
    }

    #[derive(Copy, Clone)]
    enum JumpBehavior{
        None,
        Absolute,
        Relative,
        Conditional
    }

    #[derive(PartialEq)]
    enum Register{
        EAX,
        ECX,
        EDX,
        EBX,
        ESP,
        EBP,
        ESI,
        EDI,
        AX,
        CX,
        DX,
        BX,
        SP,
        BP,
        SI,
        DI,
        AL,
        CL,
        DL,
        BL,
        AH,
        CH,
        DH,
        BH,
        Segment,
        Null
    }

    type OpcodeFn = fn();

    struct Pipeline{
        function: OpcodeFn,
        args: [OpArgument; MAX_ARGS],
        gas_cost: i32,
    }
    #[derive(Copy, Clone)]
    struct Opcode{
        function: OpcodeFn,
        arg_size: [ValueSize; MAX_ARGS],
        arg_source: [ValueSource; MAX_ARGS],
        has_modrm: bool,
        gas_cost: i32,
        rep_valid: bool,
        size_override_valid: bool,
        address_override_valid: bool,
        jump_behavior: JumpBehavior
    }

    #[derive(PartialEq)]
    enum ValueLocation{
        None,
        Immediate(u32),
        Address(u32),
        ComplexAddress{
            address: u32, 
            base: Register,
            scale: u8, //0, 1, 2, or 4
            index: Register 
        },
        ComplexImmediateAddress{
            immediate: u32, 
            base: Register, 
            scale: u8, //0, 1, 2, or 4
            index: Register 
        }
    }

    struct OpArgument{
        location: ValueLocation,
        size: ValueSize
    }


    fn nop(){

    }

    impl Default for Opcode{
        fn default() -> Opcode{
            Opcode{
                function: nop,
                arg_size: [ValueSize::None, ValueSize::None, ValueSize::None],
                arg_source: [ValueSource::None, ValueSource::None, ValueSource::None],
                has_modrm: false,
                gas_cost: 0,
                rep_valid: false,
                size_override_valid: false,
                address_override_valid: false,
                jump_behavior: JumpBehavior::None
            }
        }
    }
    impl Default for OpArgument{
        fn default() -> OpArgument{
            OpArgument{
                location: ValueLocation::None,
                size: ValueSize::None
            }
        }
    }

    lazy_static! {
        static ref OPCODES: [Opcode; 0x1FFF] = {
            let mut o: [Opcode; 0x1FFF] = [Opcode::default(); 0x1FFF];
            o[0].gas_cost = 10;
            o
        };
    }

    //Rust doesn't support bitfields directly (and workarounds are not great)
    //So, just unpack into bytes
    struct ModRM{
        mode: u8, //actually called mod, but can't call it that here
        reg: u8,
        rm: u8
    }

    fn convert_reg_to_address(reg: u8, size: ValueSize) -> u32{
        0
    }

    fn decode_args(opcode: &Opcode, bytestream: &[u8], args: &mut [OpArgument; MAX_ARGS], address_override: bool) -> Result<usize, usize>{
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

}