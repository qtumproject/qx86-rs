#[macro_use]
extern crate lazy_static;

#[allow(dead_code)] //remove after design stuff is done
mod qx86{

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
        ImmediateAddress,
        ImmediateValue,
        Register,
        RegisterAddress
    }

    #[derive(Copy, Clone)]
    enum JumpBehavior{
        None,
        Absolute,
        Relative,
        Conditional
    }

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
        gas_cost: i32,
        rep_valid: bool,
        size_override_valid: bool,
        address_override_valid: bool,
        jump_behavior: JumpBehavior
    }

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
                gas_cost: 0,
                rep_valid: false,
                size_override_valid: false,
                address_override_valid: false,
                jump_behavior: JumpBehavior::None
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

}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
