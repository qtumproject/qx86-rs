use crate::structs::*;
use crate::vm::*;
use crate::pipeline::*;

#[allow(dead_code)] //remove after design stuff is done

pub type OpcodeFn = fn(vm: &mut VM, pipeline: &Pipeline);

pub enum OpcodeError{
    InvalidOpcode,
}

//Defines how to decode the argument of an opcode
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

#[derive(PartialEq)]
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
    pub jump_behavior: JumpBehavior
}

pub fn nop(_vm: &mut VM, _pipeline: &Pipeline){

}
pub fn op_undefined(vm: &mut VM, _pipeline: &Pipeline){
    if vm.errored.is_none() {
        //if there was a previous error, don't overwrite the info
        vm.errored = Some(OpcodeError::InvalidOpcode);
        vm.error_eip = vm.eip;
    }
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
            jump_behavior: JumpBehavior::None
        }
    }
}
pub const OPCODE_TABLE_SIZE:usize = 0x1FFF;
const OP_TWOBYTE:usize = 1 << 12;
const OP_OVERRIDE:usize = 1 << 13;
const OP_GROUP_SHIFT:u8 = 8;

//helper functions for opcode map
fn with_override(op: usize) -> usize{
    op | OP_OVERRIDE
}
fn two_byte(op: usize) -> usize{
    op | OP_TWOBYTE
}
fn with_group(op:usize, group: usize) -> usize{
    if(group > 7) {
        panic!("Group opcode error in opcode initialization");
    }
    op | (group << OP_GROUP_SHIFT)
}
fn fill_groups(ops: &mut [Opcode; 0x1FFF], op:usize){
    for n in 0..8 {
        ops[with_group(op, n)] = ops[op];
    }
}
fn fill_override(ops: &mut [Opcode; 0x1FFF], op:usize){
    ops[with_override(op)] = ops[op];
}
fn fill_override_groups(ops: &mut [Opcode; 0x1FFF], op:usize){
    fill_groups(ops, op);
    fill_override(ops, op);
    fill_groups(ops, with_override(op));
}
//(Eventually) huge opcode map
lazy_static! {
    pub static ref OPCODES: [Opcode; 0x1FFF] = {
        let mut o: [Opcode; 0x1FFF] = [Opcode::default(); 0x1FFF];
        //nop
        o[0x90].function = nop;
        o[0x90].gas_cost = 0;
        fill_override_groups(&mut o, 0x90);
        


        o
    };
}

