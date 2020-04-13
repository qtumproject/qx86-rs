
#[allow(dead_code)] //remove after design stuff is done

use crate::vm::*;

pub const MAX_ARGS:usize = 3;

/// ValueSize is an enum to indicate literally the sized of a value 
#[derive(PartialEq)]
#[derive(Copy, Clone)]
#[derive(Debug)]
pub enum ValueSize{
    /// No value size, this resolves to 0 or an error depending on context
    None,
    /// Byte value size, this is 1 byte
    Byte,
    /// Word value size, this is 2 bytes
    Word,
    /// Dword value size, this is 4 bytes
    Dword,
    /// Qword value size, this is 8 bytes
    Qword,
}





/// SizedValue is a fixed size value of a determined size
#[derive(PartialEq)]
#[derive(Copy, Clone)]
#[derive(Debug)]
pub enum SizedValue{
    /// No value present, this resolves to 0 or an error depending on context
    None,
    /// A byte value
    Byte(u8),
    /// A word value of 2 bytes
    Word(u16),
    /// A dword value of 4 bytes
    Dword(u32),
    /// A quadruple word value of 8 bytes
    Qword(u64)
}

impl SizedValue{
    /// Unwraps the value expecting it to be exactly a Dword. Returns an error if not
    pub fn u64_exact(&self) -> Result<u64, VMError>{
        match self{
            SizedValue::Qword(v) => Ok(*v),
            _ => Err(VMError::WrongSizeExpectation)
        }
    }
    /// Unwraps the value expecting it to be exactly a Dword. Returns an error if not
    pub fn u32_exact(&self) -> Result<u32, VMError>{
        match self{
            SizedValue::Dword(v) => Ok(*v),
            _ => Err(VMError::WrongSizeExpectation)
        }
    }
    /// Unwraps the value expecting it to be exactly a Word. Returns an error if not
    pub fn u16_exact(&self) -> Result<u16, VMError>{
        match self{
            SizedValue::Word(v) => Ok(*v),
            _ => Err(VMError::WrongSizeExpectation)
        }
    }
    /// Unwraps the value expecting it to be exactly a Byte. Returns an error if not
    pub fn u8_exact(&self) -> Result<u8, VMError>{
        match self{
            SizedValue::Byte(v) => Ok(*v),
            _ => Err(VMError::WrongSizeExpectation)
        }
    }

    /// Unwraps the value as a u32 by zero-extending smaller values.
    /// Returns an error if the present value is too large to fit within a u32
    pub fn u32_zx(&self) ->  Result<u32, VMError>{
        match self{
            SizedValue::Dword(v) => Ok(*v),
            SizedValue::Word(v) => Ok(*v as u32),
            SizedValue::Byte(v) => Ok(*v as u32),
            SizedValue::Qword(v) => Err(VMError::TooBigSizeExpectation),
            SizedValue::None => Ok(0),
        }
    }

    /// Unwraps the value as a u16 by zero-extending smaller values.
    /// Returns an error if the present value is too large to fit within a u16
    pub fn u16_zx(&self) ->  Result<u16, VMError>{
        match self{
            SizedValue::Word(v) => Ok(*v),
            SizedValue::Byte(v) => Ok(*v as u16),
            SizedValue::None => Ok(0),
            SizedValue::Dword(_v) => Err(VMError::TooBigSizeExpectation),
            SizedValue::Qword(v) => Err(VMError::TooBigSizeExpectation),
        }
    }

    /// Unwraps the value as a u32 by sign-extending smaller values.
    /// Returns an error if the present value is too large to fit within a u32
    pub fn u32_sx(&self) ->  Result<u32, VMError>{
        match self{
            SizedValue::Dword(v) => Ok(*v),
            SizedValue::Word(v) => Ok(*v as i16 as i32 as u32),
            SizedValue::Byte(v) => Ok(*v as i8 as i32 as u32),
            SizedValue::None => Ok(0),
            SizedValue::Qword(v) => Err(VMError::TooBigSizeExpectation),
        }
    }

    /// Unwraps the value as a u16 by sign-extending smaller values.
    /// Returns an error if the present value is too large to fit within a u16
    pub fn u16_sx(&self) ->  Result<u16, VMError>{
        match self{
            SizedValue::Word(v) => Ok(*v),
            SizedValue::Byte(v) => Ok(*v as i8 as i16 as u16),
            SizedValue::None => Ok(0),
            SizedValue::Dword(_v) => Err(VMError::TooBigSizeExpectation),
            SizedValue::Qword(_v) => Err(VMError::TooBigSizeExpectation),
        }
    }

    /// Unwraps the value as a u32 by zero-extending smaller values and truncating larger values to keep only the least significant bits that will fit
    pub fn u64_trunc(&self) ->  u64{
        match self{
            SizedValue::Dword(v) => *v as u64,
            SizedValue::Qword(v) => *v,
            SizedValue::Word(v) => *v as u64,
            SizedValue::Byte(v) => *v as u64,
            SizedValue::None => 0,
        }
    }

    /// Unwraps the value as a u32 by zero-extending smaller values and truncating larger values to keep only the least significant bits that will fit
    pub fn u32_trunc(&self) ->  u32{
        match self{
            SizedValue::Dword(v) => *v,
            SizedValue::Qword(v) => *v as u32,
            SizedValue::Word(v) => *v as u32,
            SizedValue::Byte(v) => *v as u32,
            SizedValue::None => 0,
        }
    }
    /// Unwraps the value as a u16 by zero-extending smaller values and truncating larger values to keep only the least significant bits that will fit
    pub fn u16_trunc(&self) ->  u16{
        match self{
            SizedValue::Word(v) => *v as u16,
            SizedValue::Byte(v) => *v as u16,
            SizedValue::None => 0,
            SizedValue::Dword(v) => *v as u16,
            SizedValue::Qword(v) => *v as u16,
        }
    }
    /// Unwraps the value as a u8 by zero-extending smaller values and truncating larger values to keep only the least significant bits that will fit
    pub fn u8_trunc(&self) -> u8{
        match self{
            SizedValue::Word(v) => *v as u8,
            SizedValue::Byte(v) => *v as u8,
            SizedValue::None => 0,
            SizedValue::Dword(v) => *v as u8,
            SizedValue::Qword(v) => *v as u8,
        }
    }

    /// This will convert the current SizedValue to the specified ValueSize by zero-extending smaller values
    /// This will return an error if the current value will not fit into the target ValueSize
    pub fn convert_size_zx(&self, s: ValueSize) -> Result<SizedValue, VMError>{
        use ValueSize::*;
        match s{
            Dword => Ok(SizedValue::Dword(self.u32_zx()?)),
            Word => Ok(SizedValue::Word(self.u16_zx()?)),
            Byte => Ok(SizedValue::Byte(self.u8_exact()?)),
            None => Err(VMError::WrongSizeExpectation),
            Qword => Err(VMError::TooBigSizeExpectation),
        }
    }
    /// This will convert the current SizedValue to the specified ValueSize by sign-extending smaller values
    /// This will return an error if the current value will not fit into the target ValueSize
    pub fn convert_size_sx(&self, s: ValueSize) -> Result<SizedValue, VMError>{
        use ValueSize::*;
        match s{
            Dword => Ok(SizedValue::Dword(self.u32_sx()?)),
            Word => Ok(SizedValue::Word(self.u16_sx()?)),
            Byte => Ok(SizedValue::Byte(self.u8_exact()?)),
            None => Err(VMError::WrongSizeExpectation),
            Qword=> Err(VMError::TooBigSizeExpectation),
        }
    }
    /// This will convert the current SizedValue to the specified ValueSize by zero-extending smaller values and truncating larger values than will fit
    pub fn convert_size_trunc(&self, s: ValueSize) -> SizedValue{
        use ValueSize::*;
        match s{
            Qword => SizedValue::Qword(self.u64_trunc()),
            Dword => SizedValue::Dword(self.u32_trunc()),
            Word => SizedValue::Word(self.u16_trunc()),
            Byte => SizedValue::Byte(self.u8_trunc()),
            None => SizedValue::None
        }
    }
}

trait ToSizedValue{
    /// Will convert the current value to a SizedValue of appropriate size
    /// This trait should not be implemented on types which may not fit within a SizedValue
    fn to_sized(&self) -> SizedValue;
}
impl ToSizedValue for u8{
    fn to_sized(&self) -> SizedValue{
        return SizedValue::Byte(*self);
    }
}
impl ToSizedValue for u16{
    fn to_sized(&self) -> SizedValue{
        return SizedValue::Word(*self);
    }
}
impl ToSizedValue for u32{
    fn to_sized(&self) -> SizedValue{
        return SizedValue::Dword(*self);
    }
}

/// ArgLocation is used to specify how to locate an opcode's argument
/// It is a rich enum which specifies everything from different forms of addressing, to immediate values and register values
/// It is difficult to use directly, so a SizedValue can be retreived by using `VM::get_arg` to resolve the location
#[derive(PartialEq)]
#[derive(Copy, Clone)]
#[derive(Debug)]
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

    //note offset here can be negative (ie, top bit set) but with wrapping_add
    //the results will be identical without needing to juggle between u32 and i32 with type checks etc
    ModRMAddress{
        offset: Option<u32>,
        reg: Option<u8>,
        size: ValueSize
    },
    SIBAddress{
        offset: u32,
        base: Option<u8>, //register
        scale: u8, //1, 2, 4, 8
        index: Option<u8>,
        size: ValueSize
    }
}

impl Default for ArgLocation{
    fn default() -> ArgLocation{
        ArgLocation::None
    }
}

/// Specifies information about an opcode's argument within execution such as the location and if that location is within memory 
#[derive(Copy, Clone)]
pub struct OpArgument{
    pub location: ArgLocation,
    pub is_memory: bool
}





impl Default for OpArgument{
    fn default() -> OpArgument{
        OpArgument{
            location: ArgLocation::None,
            is_memory: false
        }
    }
}

