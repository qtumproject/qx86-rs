
#[allow(dead_code)] //remove after design stuff is done

pub const MAX_ARGS:usize = 3;

#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum ValueSize{
    None,
    Byte,
    Word,
    Dword
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
#[derive(Copy, Clone)]
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


#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum SizedValue{
    None,
    Byte(u8),
    Word(u16),
    Dword(u32)
}

#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum ArgLocation{
    None,
    Immediate(SizedValue),
    Address(u32, ValueSize), //an immediate address
    RegisterValue(u8, ValueSize),
    RegisterAddress(u8, ValueSize),
    ModRMAddress16{
        offset: Option<u16>, 
        reg1: Option<u8>,
        reg2: Option<u8>
    },
    ModRMAddress32{
        offset: Option<u32>,
        reg: Option<u8>
    },
    SIBAddress32{
        offset: Option<u32>,
        base: Option<u8>, //register
        scale: u8, //1, 2, 4, 8
        index: Option<u8>
    }
}

#[derive(Copy, Clone)]
pub struct OpArgument{
    pub location: ArgLocation,
    pub size: u8 //size in bytes
}





impl Default for OpArgument{
    fn default() -> OpArgument{
        OpArgument{
            location: ArgLocation::None,
            size: 0
        }
    }
}

