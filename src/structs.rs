
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

impl SizedValue{
    pub fn u32(&self) -> u32{
        match self{
            SizedValue::Dword(v) => *v,
            _ => panic!("Invalid SizedValue expectation")
        }
    }
    pub fn u16(&self) -> u16{
        match self{
            SizedValue::Word(v) => *v,
            _ => panic!("Invalid SizedValue expectation")
        }
    }
    pub fn u8(&self) -> u8{
        match self{
            SizedValue::Byte(v) => *v,
            _ => panic!("Invalid SizedValue expectation")
        }
    }
}

#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum ArgLocation{
    None,
    Immediate(SizedValue),
    Address(u32, ValueSize), //an immediate address
    RegisterValue(u8, ValueSize),
    RegisterAddress(u8, ValueSize),
    /*ModRMAddress16{ //Not supported except for with LEA
        offset: Option<u16>, 
        reg1: Option<u8>,
        reg2: Option<u8>,
        size: ValueSize
    }, */
    ModRMAddress{
        offset: Option<u32>,
        reg: Option<u8>,
        size: ValueSize
    },
    SIBAddress{
        offset: Option<u32>,
        base: Option<u8>, //register
        scale: u8, //1, 2, 4, 8
        index: Option<u8>,
        size: ValueSize
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

