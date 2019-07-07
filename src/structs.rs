use crate::opcodes::OpcodeFn;

pub const MAX_ARGS:usize = 3;

#[derive(Copy, Clone)]
pub enum ValueSize{
    None,
    Byte,
    Word,
    DWord
}

#[derive(Copy, Clone)]
pub enum ValueSource{
    None,
    ModRM,
    ModRMReg, //the /r field
    ImmediateAddress,
    ImmediateValue,
    RegisterSuffix //lowest 3 bits of the opcode is used for register
}

#[derive(Copy, Clone)]
pub enum JumpBehavior{
    None,
    Absolute,
    Relative,
    Conditional
}

#[derive(PartialEq)]
pub enum Register{
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

pub struct Pipeline{
    pub function: OpcodeFn,
    pub args: [OpArgument; MAX_ARGS],
    pub gas_cost: i32,
}


#[derive(PartialEq)]
pub enum ValueLocation{
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

pub struct OpArgument{
    pub location: ValueLocation,
    pub size: ValueSize
}





impl Default for OpArgument{
    fn default() -> OpArgument{
        OpArgument{
            location: ValueLocation::None,
            size: ValueSize::None
        }
    }
}

