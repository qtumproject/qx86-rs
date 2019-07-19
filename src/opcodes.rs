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
    //This is treated the same as ImmediateValue, but specialized so that the Pipeline can interpret it directly
    //without requiring a full decode and execution
    JumpRel,
    Literal(SizedValue), //For encoding hard-coded values, such as the `rol modrm8, 1` opcode
    HardcodedRegister(u8) //for encoding hard-coded registers, such as `mov EAX, offs32`
}
#[derive(Copy, Clone)]
pub enum OpcodeValueSize{
    Fixed(ValueSize),
    NativeWord //this translates into either Word or Dword depending on if an operand size override prefix is present
}

impl OpcodeValueSize{
    pub fn to_fixed(&self, size_override: bool) -> ValueSize{
        match self{
            OpcodeValueSize::Fixed(f) => *f,
            OpcodeValueSize::NativeWord => {
                if size_override{
                    ValueSize::Word
                }else{
                    ValueSize::Dword
                }
            }
        }
    }
}

#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum PipelineBehavior{
    None,
    //This is for predictable jumps with a hard coded jump target
    RelativeJump,
    //any opcode which changes EIP or execution state and can not be predicted at the decoding stage
    //this includes opcodes like `jne` and also opcodes like `jmp eax`, as well as system calls using `int`
    Unpredictable 
}

//defines an opcode with all the information needed for decoding the opcode and all arguments
#[derive(Copy, Clone)]
pub struct Opcode{
    pub function: OpcodeFn,
    pub arg_size: [OpcodeValueSize; MAX_ARGS],
    pub arg_source: [ArgSource; MAX_ARGS],
    pub gas_cost: GasCost,
    pub pipeline_behavior: PipelineBehavior,
    pub defined: bool
}

#[derive(Copy, Clone)]
pub struct OpcodeProperties{
    pub has_modrm: bool,
    pub defined: bool,
    //pub rep_valid: bool, //this is handled in decoding by special case checking -- 0xA4 through 0xAF, excluding 0xA8 and 0xA9

    //0 is the normal opcode, while the entire array is used for "group" opcodes which use the reg
    //field of Mod R/M to extend the opcode
    //For "/r" opcodes which use the reg field as an additional parameter, the opcode is duplicated to fill this entire array
    pub opcodes: [Opcode; 8],
}

pub fn nop(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
Ok(())
}
pub fn op_undefined(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
    Err(VMError::InvalidOpcode)
}

impl Default for OpcodeProperties{
    fn default() -> OpcodeProperties{
        OpcodeProperties{
            has_modrm: false,
            defined: false,
            opcodes: [Opcode::default(); 8],
        }
    }
}

impl Default for Opcode{
    fn default() -> Opcode{
        Opcode{
            function: op_undefined,
            arg_size: [OpcodeValueSize::Fixed(ValueSize::None); 3],
            arg_source: [ArgSource::None; 3],
            gas_cost: GasCost::None,
            //this defaults to conditional so that an unknown opcode is considered conditional
            pipeline_behavior: PipelineBehavior::Unpredictable,
            defined: false
        }
    }
}

//index: lower byte is primary opcode
//upper bit is if 0x0F prefix is used (ie, extended opcode)
pub const OPCODE_TABLE_SIZE:usize = 0x1FF;
const OP_TWOBYTE:usize = 1 << 8;


#[derive(Default)]
pub struct OpcodeDefiner{
    opcode: u8,
    //None = handle both with/without size override, false = without size override, true = with size override
    two_byte: bool,
    group: Option<u8>,
    gas_level: Option<GasCost>,
    args: Vec<(ArgSource, OpcodeValueSize)>,
    function: Option<OpcodeFn>,
    jump: Option<PipelineBehavior>,
    has_modrm: bool,
    reg_suffix: bool
}

impl OpcodeDefiner{
    pub fn is_group(&mut self, group: u8) -> &mut OpcodeDefiner{
        self.group = Some(group);
        self.has_modrm = true;
        self
    }
    pub fn calls(&mut self, function: OpcodeFn) -> &mut OpcodeDefiner{
        self.function = Some(function);
        self
    }
    pub fn with_arg(&mut self, source: ArgSource, size: OpcodeValueSize) -> &mut OpcodeDefiner{
        
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
    pub fn is_jump(&mut self) -> &mut OpcodeDefiner{
        self.jump = Some(PipelineBehavior::RelativeJump);
        self
    }
    pub fn is_unpredictable(&mut self) -> &mut OpcodeDefiner{
        self.jump = Some(PipelineBehavior::Unpredictable);
        self
    }
    pub fn with_gas(&mut self, gas: GasCost) -> &mut OpcodeDefiner{
        self.gas_level = Some(gas);
        self
    }
    //arg helpers to keep sanity when defining opcodes
    pub fn with_rm8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRM, OpcodeValueSize::Fixed(ValueSize::Byte))
    }   
    pub fn with_rmw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRM, OpcodeValueSize::NativeWord)
    }
    pub fn with_rm_regw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRMReg, OpcodeValueSize::NativeWord)
    }
    pub fn with_rm_reg8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRMReg, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    pub fn with_imm8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateValue, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    pub fn with_immw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateValue, OpcodeValueSize::NativeWord)
    }
    pub fn with_offs8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateAddress, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    pub fn with_offsw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateAddress, OpcodeValueSize::NativeWord)
    }
    pub fn with_suffix_reg8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::RegisterSuffix, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    pub fn with_suffix_regw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::RegisterSuffix, OpcodeValueSize::NativeWord)
    }

    pub fn into_table(&mut self, table: &mut [OpcodeProperties]){
        if self.jump.is_none(){
            self.jump = Some(PipelineBehavior::None);
        }
        if self.gas_level.is_none(){
            self.gas_level = Some(GasCost::Low);
        }
        let limit = if self.reg_suffix{
            8
        }else{
            1
        };
        for n in 0..limit {
            let mut op = self.opcode as usize + n;
            op = match self.two_byte{
                true => op | OP_TWOBYTE,
                false => op
            };
            if table[op].defined && !table[op].has_modrm{
                panic!("Conflicting opcode definition detected");
            }
            table[op].defined = true;
            table[op].has_modrm = self.has_modrm;

            //write to all 8 inner opcodes if has_modrm and no group
            //Otherwise write to just the group inner opcode
            let op_limit = if self.has_modrm && self.group.is_none(){
                8
            }else{
                1
            };
            let op_start = self.group.unwrap_or(0) as usize;
            for inner in op_start..op_limit{
                if table[op].opcodes[inner].defined {
                    panic!("Conflicting opcode definition detected");
                }
                table[op].opcodes[inner].defined = true;
                table[op].opcodes[inner].function = self.function.unwrap();
                table[op].opcodes[inner].gas_cost = self.gas_level.unwrap();
                table[op].opcodes[inner].pipeline_behavior = self.jump.unwrap();
                for n in 0..self.args.len(){
                    let (source, size) = self.args[n];
                    table[op].opcodes[inner].arg_source[n] = source;
                    table[op].opcodes[inner].arg_size[n] = size;
                }
            }
        }
    }
}
pub fn define_opcode(opcode: u8) -> OpcodeDefiner{
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

Use a suffix of "W" in comments to indicate that an argument can be either word or dword depending on if there is an operand size override

*/


//(Eventually) huge opcode map
lazy_static! {
    pub static ref OPCODES: [OpcodeProperties; OPCODE_TABLE_SIZE] = {
        use crate::ops::*;
        use OpcodeValueSize::*;
        use ValueSize::*;
        use ArgSource::*;
        use GasCost::*;
        let mut ops: [OpcodeProperties; OPCODE_TABLE_SIZE] = [OpcodeProperties::default(); OPCODE_TABLE_SIZE];
        //nop
        define_opcode(0x90).calls(nop).with_gas(GasCost::None).into_table(&mut ops);

        //hlt
        define_opcode(0xF4).calls(hlt).with_gas(GasCost::None).is_unpredictable().into_table(&mut ops);

        //mov opcodes
        //0xB0 mov r8, imm8
        define_opcode(0xB0).calls(mov).with_gas(VeryLow)
            .with_suffix_reg8()
            .with_imm8()
            .into_table(&mut ops);
        //0xB8 mov rW, immW
        define_opcode(0xB8).calls(mov).with_gas(VeryLow)
            .with_suffix_regw()
            .with_immw()
            .into_table(&mut ops);
        //0x88 /r mov rm8, r8
        define_opcode(0x88).calls(mov).with_gas(VeryLow)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x89 /r mov rmW, rW       
        define_opcode(0x89).calls(mov).with_gas(VeryLow)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x8A /r mov r8, rm8
        define_opcode(0x8A).calls(mov).with_gas(VeryLow)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops); 
        //0x8B /r mov rW, rmW
        define_opcode(0x8B).calls(mov).with_gas(VeryLow)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops);
        //0xA0 mov AL, offs8
        define_opcode(0xA0).calls(mov).with_gas(VeryLow)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_offs8()
            .into_table(&mut ops);
        //0xA1 mov EAX/AX, offsW
        define_opcode(0xA1).calls(mov).with_gas(VeryLow)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_offsw()
            .into_table(&mut ops);
        //0xA2 mov offs8, AL
        define_opcode(0xA2).calls(mov).with_gas(VeryLow)
            .with_offs8()
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .into_table(&mut ops);
        //0xA3 mov offsW, EAX/AX
        define_opcode(0xA3).calls(mov).with_gas(VeryLow)
            .with_offsw()
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .into_table(&mut ops);
        //0xC6 mov rm8, imm8
        define_opcode(0xC6).calls(mov).with_gas(VeryLow)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0xC7 mov rmW, immW
        define_opcode(0xC7).calls(mov).with_gas(VeryLow)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);


        ops
    };
}

