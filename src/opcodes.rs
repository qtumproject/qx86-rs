use crate::structs::*;
use crate::vm::*;
use crate::pipeline::*;

#[allow(dead_code)] //remove after design stuff is done

pub type OpcodeFn = fn(vm: &mut VM, pipeline: &Pipeline);

pub enum OpcodeError{

}

#[derive(Copy, Clone)]
pub struct Opcode{
    pub function: OpcodeFn,
    pub arg_size: [ValueSize; MAX_ARGS],
    pub arg_source: [ValueSource; MAX_ARGS],
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

//(Eventually) huge opcode map
lazy_static! {
    static ref OPCODES: [Opcode; 0x1FFF] = {
        let mut o: [Opcode; 0x1FFF] = [Opcode::default(); 0x1FFF];
        o[0].gas_cost = 10;
        o
    };
}

