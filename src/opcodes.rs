use crate::structs::*;
use crate::vm::*;
use crate::pipeline::*;

#[allow(dead_code)] //remove after design stuff is done

pub type OpcodeFn = fn(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>;

//Defines how to decode the argument of an opcode
#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum ArgSource{
    None,
    ModRM,
    ModRMReg, //the /r field
    ImmediateValue,
    ImmediateAddress, //known as an "offset" in docs rather than pointer or address
    RegisterSuffix, //lowest 3 bits of the opcode is used for register
    //note: for Jump opcodes, exactly 1 argument is the only valid encoding
    JumpRel
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
    pub jump_behavior: JumpBehavior,
    pub defined: bool
}

pub fn nop(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
Ok(())
}
pub fn op_undefined(vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
    Err(VMError::InvalidOpcode)
}

impl Default for Opcode{
    fn default() -> Opcode{
        Opcode{
            function: op_undefined,
            arg_size: [ValueSize::None, ValueSize::None, ValueSize::None],
            arg_source: [ArgSource::None, ArgSource::None, ArgSource::None],
            has_modrm: false,
            gas_cost: 0,
            rep_valid: false,
            size_override_valid: false,
            jump_behavior: JumpBehavior::None,
            defined: false
        }
    }
}
pub const OPCODE_TABLE_SIZE:usize = 0x1FFF;
const OP_TWOBYTE:usize = 1 << 11;
const OP_OVERRIDE:usize = 1 << 12;
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
fn fill_groups(ops: &mut [Opcode], op:usize){
    for n in 0..8 {
        ops[with_group(op, n)] = ops[op];
    }
}
fn fill_override(ops: &mut [Opcode], op:usize){
    ops[with_override(op)] = ops[op];
}
fn fill_override_groups(ops: &mut [Opcode], op:usize){
    fill_groups(ops, op);
    fill_override(ops, op);
    fill_groups(ops, with_override(op));
}



#[derive(Default)]
struct OpcodeDefiner{
    opcode: u8,
    //None = handle both with/without size override, false = without size override, true = with size override
    size_override: Option<bool>,
    two_byte: bool,
    group: Option<u8>,
    gas_level: u32, //todo, make enum?
    args: Vec<(ArgSource, ValueSize)>,
    function: Option<OpcodeFn>,
    jump: Option<JumpBehavior>,
    has_modrm: bool,
    has_rep: bool,
    reg_suffix: bool
}

impl OpcodeDefiner{
    fn is_group(&mut self, group: u8) -> &mut OpcodeDefiner{
        self.group = Some(group);
        self.has_modrm = true;
        self
    }
    fn calls(&mut self, function: OpcodeFn) -> &mut OpcodeDefiner{
        self.function = Some(function);
        self
    }
    fn with_override(&mut self) -> &mut OpcodeDefiner{
        self.size_override = Some(true);
        self
    }
    fn without_override(&mut self) -> &mut OpcodeDefiner{
        self.size_override = Some(false);
        self
    }
    fn has_arg(&mut self, source: ArgSource, size: ValueSize) -> &mut OpcodeDefiner{
        
        let hasmodrm = match source{
            ArgSource::ModRMReg | ArgSource::ModRM => true,
            _ => false
        };
        self.has_modrm |= hasmodrm;
        if source == ArgSource::RegisterSuffix{
            self.reg_suffix = true;
        }
        self.args.push((source, size));
        self
    }
    fn is_jump(&mut self) -> &mut OpcodeDefiner{
        self.jump = Some(JumpBehavior::Relative);
        self
    }
    fn is_conditional(&mut self) -> &mut OpcodeDefiner{
        self.jump = Some(JumpBehavior::Conditional);
        self
    }
    fn with_rep(&mut self) -> &mut OpcodeDefiner{
        self.has_rep = true;
        self
    }
    fn with_gas(&mut self, gas: u32) -> &mut OpcodeDefiner{
        self.gas_level = gas;
        self
    }

    fn into_table(&mut self, table: &mut [Opcode]){
        if self.jump.is_none(){
            self.jump = Some(JumpBehavior::None);
        }
        let limit = if self.reg_suffix{
            8
        }else{
            1
        };
        for n in 0..limit {
            let mut op = self.opcode as usize + n;
            op = match self.size_override{
                None => op,
                Some(x) => {
                    if x{
                        with_override(op)
                    }else{
                        op
                    }
                }
            };
            table[op].defined = true;
            table[op].function = self.function.unwrap();
            table[op].gas_cost = self.gas_level as i32;
            table[op].jump_behavior = self.jump.unwrap();
            for n in 0..self.args.len(){
                let (source, size) = self.args[n];
                table[op].arg_source[n] = source;
                table[op].arg_size[n] = size;
            }
            table[op].has_modrm = self.has_modrm;
            table[op].rep_valid = self.has_rep;
            if self.group.is_none() {
                fill_groups(table, op);
            }
            //todo: add group stuff for else
            if self.size_override.is_none(){
                //if none, then assume this opcode defines both with and without override versions
                fill_override(table, op);
                if !self.group.is_none(){
                    fill_groups(table, with_override(op));
                }
            }else{
                if self.size_override.unwrap(){
                    if !self.group.is_none(){
                        fill_groups(table, op);
                    }
                }
            }
        }
    }
    
}
fn define_opcode(opcode: u8) -> OpcodeDefiner{
    let mut d = OpcodeDefiner::default();
    //d.args.resize(3, (ArgSource::None, ValueSize::None));
    d.opcode = opcode;
    d
}

/*
Opcode definition convention note:
This uses the format used by Intel assembly syntax
Thus, the first argument would be the destination for opcodes which have an explicit destination
For instance, an opcode like this:
mov eax, 0x12345678

Would be defined by saying the first argument is a register (or modrm) and the second argument is an immediate
In addition, all test code comments and other annotations should use the intel assembly syntax, and NOT the GNU "AT&T" syntax
*/


//(Eventually) huge opcode map
lazy_static! {
    pub static ref OPCODES: [Opcode; OPCODE_TABLE_SIZE] = {
        use crate::ops::*;
        let mut ops: [Opcode; OPCODE_TABLE_SIZE] = [Opcode::default(); OPCODE_TABLE_SIZE];
        //nop
        define_opcode(0x90).calls(nop).with_gas(0).into_table(&mut ops);

        //hlt
        define_opcode(0xF4).calls(hlt).with_gas(0).is_conditional().into_table(&mut ops);

        //mov r8, imm8
        define_opcode(0xB0).calls(mov).with_gas(10).
            has_arg(ArgSource::RegisterSuffix, ValueSize::Byte).
            has_arg(ArgSource::ImmediateValue, ValueSize::Byte).
            into_table(&mut ops);


        ops
    };
}

