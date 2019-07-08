use crate::structs::*;
use crate::vm::*;
use crate::pipeline::*;

#[allow(dead_code)] //remove after design stuff is done

pub type OpcodeFn = fn(vm: &mut VM, pipeline: &Pipeline);

pub enum OpcodeError{

}

#[derive(Copy, Clone)]
pub enum ArgSource{
    None,
    ModRM,
    ModRMReg, //the /r field
    ImmediateValue,
    ImmediateAddress, //known as an "offset" in docs rather than pointer or address
    RegisterSuffix, //lowest 3 bits of the opcode is used for register
    //note: for Jump opcodes, exactly 1 argument is the only valid encoding
    JumpRel8,
    JumpRel16,
    JumpRel32
}

#[derive(Copy, Clone)]
pub enum JumpBehavior{
    None,
    Relative,
    //any opcode which changes EIP by an amount which can not be predicted at the decoding stage
    //this includes opcodes like `jne` and also opcodes like `jmp eax` 
    Conditional 
}

//defines an opcode with all the information needed for decoding the opcode and all arguments
#[derive(Copy, Clone)]
pub struct Opcode{
    pub function: OpcodeFn,
    pub arg_size: [ValueSize; MAX_ARGS],
    pub arg_source: [ArgSource; MAX_ARGS],
    pub has_modrm: bool,
    pub gas_cost: i32,
    pub rep_valid: bool,
    pub size_override_valid: bool,
    pub address_override_valid: bool,
    pub jump_behavior: JumpBehavior
}

pub fn nop(_vm: &mut VM, _pipeline: &Pipeline){

}

impl Default for Opcode{
    fn default() -> Opcode{
        Opcode{
            function: nop,
            arg_size: [ValueSize::None, ValueSize::None, ValueSize::None],
            arg_source: [ArgSource::None, ArgSource::None, ArgSource::None],
            has_modrm: false,
            gas_cost: 0,
            rep_valid: false,
            size_override_valid: false,
            address_override_valid: false,
            jump_behavior: JumpBehavior::None
        }
    }
}

//(Eventually) huge opcode map
lazy_static! {
    static ref OPCODES: [Opcode; 0x1FFF] = {
        let mut o: [Opcode; 0x1FFF] = [Opcode::default(); 0x1FFF];
        //nop
        o[0x90].gas_cost = 0;
        
        

        o
    };
}

