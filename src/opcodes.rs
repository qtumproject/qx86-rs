use crate::structs::*;
use crate::vm::*;
use crate::pipeline::*;

#[allow(dead_code)] //remove after design stuff is done

pub type OpcodeFn = fn(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError>;

/// Defines how to decode an argument of an opcode
#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum ArgSource{
    /// Specifies that there is no argument
    None,
    /// Specifies that the argument is a Mod R/M byte. This can resolve to simple or complex addressing forms, as well as register values
    ModRM,
    /// Specifies that the argument is a register corresponding to the "reg" field of the Mod R/M byte.
    /// This is often described in x86 reference documents as being a `/r` opcode
    ModRMReg, 
    /// Specifies that the argument is an immeidate value within the opcode stream
    ImmediateValue,
    /// Specifies that the argument is an immediate address within the opcode stream
    /// This is often described as an "offset" in x86 reference documents
    ImmediateAddress,
    /// Specifies that the argument is a register chosen by the bottom 3 bits of the current opcode
    /// This is often described as a "+r" opcode in x86 reference documents
    RegisterSuffix,

    /// This is treated the same as an ImmediateValue but explicitly specified so that the pipeline building process can interpret it directly
    /// This allows the pipeline process to follow these easy to predict jumps rather than causing a pipeline termination
    JumpRel,
    /// This indicates that the argument should be treated as a hard coded value
    /// This is needed to avoid special case logic functions in some instructions, such as `rol modrm8, 1` 
    Literal(SizedValue), 
    /// This indicates that the argument should be treated as a hard coded register
    /// This is needed to avoid special case logic functions in some instructions, such as `mov EAX, offs32`
    HardcodedRegister(u8)
}

/// This is similar to the ValueSize enum, but allows for specifyng that a value is of either fixed size, 
/// or a size dependent on the presence of an operand size override prefix.
/// After decoding within the pipeline building process, this will be resolved to a fixed ValueSize
#[derive(Copy, Clone)]
pub enum OpcodeValueSize{
    /// Indicates the opcode argument is of a fixed size
    Fixed(ValueSize),
    /// Indicates that the opcode argument is a word if there is an operand size override prefix present, otherwise is a dword
    NativeWord
}

impl OpcodeValueSize{
    /// Resolves the OpcodeValueSize to a fixed ValueSize
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

/// This specifies if an instruction requires special handling within the pipeline building process
#[derive(PartialEq)]
#[derive(Copy, Clone)]
pub enum PipelineBehavior{
    /// This is for normal behavior indicating no special treatment is needed
    None,
    /// This is for predictable jumps with a hard coded jump target.
    /// The pipeline building process will follow these jumps since they are static and can be predicted
    RelativeJump,
    /// This is for any opcode which changes EIP or execution state that may affect opcodes later in the pipeline and can not easily be predicted
    /// this includes opcodes like `jne` and also opcodes like `jmp [eax]`, as well as system calls using `int`
    Unpredictable,
    /// This is for the same behavior as Unpredictable but without a gas penalty
    /// used for int and hlt, where execute state can change so pipelining can't continue,
    /// but they're not really a conditional branch that one would expect to pay an extra gas charge for
    UnpredictableNoGas,
}

/// Defines an opcode with all the information needed for decoding the opcode and its arguments
#[derive(Copy, Clone)]
pub struct Opcode{
    pub function: OpcodeFn,
    pub arg_size: [OpcodeValueSize; MAX_ARGS],
    pub arg_source: [ArgSource; MAX_ARGS],
    pub gas_cost: GasCost,
    pub pipeline_behavior: PipelineBehavior,
    pub defined: bool
}

/// This is a "super-opcode" which may have multiple child opcodes.
/// One OpcodeProperties refers to exactly one opcode byte.
/// The exact opcode is then determined by (potentially) parsing a Mod R/M byte
#[derive(Copy, Clone)]
pub struct OpcodeProperties{
    /// This super opcode has a Mod R/M byte which requires decoding
    pub has_modrm: bool,
    /// This super opcode is explicitly defined (not directly used for execution)
    pub defined: bool,
    //pub rep_valid: bool, //this is handled in decoding by special case checking -- 0xA4 through 0xAF, excluding 0xA8 and 0xA9

    /// 0 is the normal opcode, while the entire array is used for "group" opcodes which use the reg
    /// field of Mod R/M to extend the opcode
    /// For "/r" opcodes which use the reg field as an additional parameter, the opcode is duplicated to fill this entire array
    pub opcodes: [Opcode; 8],
}

pub fn nop(_vm: &mut VM, _pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
Ok(())
}

pub fn op_undefined(vm: &mut VM, _pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    Err(VMError::InvalidOpcode(vm.get_mem(vm.eip, ValueSize::Byte)?.u8_exact()?))
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

/// The master opcode table.
/// index: lower byte is primary opcode.
/// upper bit is set if 0x0F prefix is used (ie, extended opcode)
pub const OPCODE_TABLE_SIZE:usize = 0x1FF;
const OP_TWOBYTE:usize = 1 << 8;

/// This is a helper structure and set of functions for defining opcodes
/// Basically it provides sane defaults for how opcodes are typically defined and saves a lot of typing and potential errors on the programmer's part
#[derive(Default)]
pub struct OpcodeDefiner{
    opcode: u8,
    len: usize,
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
    /// Specifies that the opcode is an extended opcode and therefore needs to be read in two bytes
    pub fn is_two_byte_op(&mut self) -> &mut OpcodeDefiner {
        self.two_byte = true;
        self
    }
    /// Specifies that the opcode is a group opcode.
    /// Example: to encode `0xFF /0` one would use define_opcode(0xFF).is_group(0)
    pub fn is_group(&mut self, group: u8) -> &mut OpcodeDefiner{
        self.group = Some(group);
        self.has_modrm = true;
        self
    }
    /// Specifies which function pointer to call when the opcode is executed
    pub fn calls(&mut self, function: OpcodeFn) -> &mut OpcodeDefiner{
        self.function = Some(function);
        self
    }
    /// Specifies that the next argument for the opcode is from a particular source and of a particular size
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
    /// Specifies that the opcode should use the RelativeJump pipeline behavior
    pub fn is_jump(&mut self) -> &mut OpcodeDefiner{
        self.jump = Some(PipelineBehavior::RelativeJump);
        self
    }
    /// Specifies that the opcode is unpredictable and pipeline filling should stop upon encountering it 
    pub fn is_unpredictable(&mut self) -> &mut OpcodeDefiner{
        self.jump = Some(PipelineBehavior::Unpredictable);
        self
    }
    /// Specifies that the opcode is unpredictable, but to not charge the unpredictable gas surcharge
    pub fn is_unpredictable_no_gas(&mut self) -> &mut OpcodeDefiner{
        self.jump = Some(PipelineBehavior::UnpredictableNoGas);
        self
    }
    /// Specifies the gas tier of the opcode
    pub fn with_gas(&mut self, gas: GasCost) -> &mut OpcodeDefiner{
        self.gas_level = Some(gas);
        self
    }
    /// Specifies that the next argument is a ModRM argument of byte size
    pub fn with_rm8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRM, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    /// Specifies that the next argument is a ModRM argument of byte size
    pub fn with_rm16(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRM, OpcodeValueSize::Fixed(ValueSize::Word))
    }
    /// Specifies that the next argument is a ModRM argument of byte size
    pub fn with_rm32(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRM, OpcodeValueSize::Fixed(ValueSize::Dword))
    }
    /// Specifies that the next argument is a ModRM argument of NativeWord size
    pub fn with_rmw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRM, OpcodeValueSize::NativeWord)
    }
    /// Specifies that the next argument is a ModRM /r argument of NativeWord size
    pub fn with_rm_regw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRMReg, OpcodeValueSize::NativeWord)
    }
    /// Specifies that the next argument is a ModRM /r argument of word size
    pub fn with_rm_reg16(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRMReg, OpcodeValueSize::Fixed(ValueSize::Word))
    }
    /// Specifies that the next argument is a ModRM /r argument of dword size
    pub fn with_rm_reg32(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRMReg, OpcodeValueSize::Fixed(ValueSize::Dword))
    }
    /// Specifies that the next argument is a ModRM /r argument of byte size
    pub fn with_rm_reg8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRMReg, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    /// Specifies that the next argument is an immediate byte value
    pub fn with_imm8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateValue, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    /// Specifies that the next argument is an immediate word value
    pub fn with_imm16(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateValue, OpcodeValueSize::Fixed(ValueSize::Word))
    }
    /// Specifies that the next argument is an immeidate NativeWord value
    pub fn with_immw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateValue, OpcodeValueSize::NativeWord)
    }
    /// Specifies that the next argument is an immediate address pointing to a byte size value
    pub fn with_offs8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateAddress, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    /// Specifies that the next argument is an immediate address pointing to a NativeWord size value
    pub fn with_offsw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateAddress, OpcodeValueSize::NativeWord)
    }
    /// Specifies that the next argument is a +r register opcode suffix chosen from the byte sized register set
    pub fn with_suffix_reg8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::RegisterSuffix, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    /// Specifies that the next argument is a +r register opcode suffix chosen from the dword or word sized register set
    pub fn with_suffix_regw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::RegisterSuffix, OpcodeValueSize::NativeWord)
    }
    /// Specifies that the next argument is a +r register opcode suffix chosen from the dword or word sized register set
    pub fn with_suffix_reg32(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::RegisterSuffix, OpcodeValueSize::Fixed(ValueSize::Dword))
    }
    /// Condenses the current opcode description into the faster to execute OpcodeProperties/Opcode structures.
    /// Depending on the exact opcode description, this can fill multiple slots within the opcode table.
    /// Simple error checking is included to ensure that the same opcode is not specified twice
    pub fn into_table(&mut self, table: &mut [OpcodeProperties]){
        if self.jump.is_none(){
            self.jump = Some(PipelineBehavior::None);
        }
        if self.gas_level.is_none(){
            self.gas_level = Some(GasCost::Low);
        }
        if self.len == 0{
            panic!("Incorrect opcode configuration");
        }
        if self.reg_suffix && self.len > 1{
            panic!("Incorrect opcode configuration");
        }
        let limit = if self.reg_suffix{
            8
        }else{
            self.len
        };
        for n in 0..limit {
            let mut op = self.opcode as usize + n;
            op = match self.two_byte{
                true => op | OP_TWOBYTE,
                false => op
            };
            if op == 0x90 {
                if table[op].defined {
                    continue; // this is to support both nop and xchg
                }
            }
            if table[op].defined && !table[op].has_modrm{
                panic!("Conflicting opcode definition detected");
            }
            table[op].defined = true;
            table[op].has_modrm = self.has_modrm;

            //write to all 8 inner opcodes if has_modrm and no group
            //Otherwise write to just the group inner opcode
            let op_limit = if self.has_modrm{
                if self.group.is_none(){
                    8
                }else{
                    self.group.unwrap() as usize + 1
                }
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
/// Defines a new opcode and creates a new OpcodeDefiner helper struct
pub fn define_opcode(opcode: u8) -> OpcodeDefiner{
    let mut d = OpcodeDefiner::default();
    d.opcode = opcode;
    d.len = 1;
    d
}
pub fn define_opcode_multi(opcode: u8, len: usize) -> OpcodeDefiner{
    let mut d = OpcodeDefiner::default();
    d.opcode = opcode;
    d.len = len;
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


lazy_static! {
    /// The master qx86 subset opcode map definition.
    /// Note this uses lazy_static so that the definitions can be constructed more simply while not incurring a runtime execution cost
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
        define_opcode(0xF4).calls(hlt).with_gas(GasCost::None).is_unpredictable_no_gas().into_table(&mut ops);

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
        //push opcodes
        //0x50 +r push rW
        define_opcode(0x50).calls(push).with_gas(VeryLow)
            .with_suffix_regw()
            .into_table(&mut ops);
        //0x68 push immW
        define_opcode(0x68).calls(push).with_gas(VeryLow)
            .with_immw()
            .into_table(&mut ops);
        //0x6A push imm8
        define_opcode(0x6A).calls(push).with_gas(VeryLow)
            .with_imm8()
            .into_table(&mut ops);
        //0xFF /6 push rmW
        define_opcode(0xFF).calls(push).with_gas(VeryLow)
            .is_group(6)
            .with_rmw()
            .into_table(&mut ops);
        //pop opcodes
        //0x58 +r pop rW
        define_opcode(0x58).calls(pop).with_gas(VeryLow)
            .with_suffix_regw()
            .into_table(&mut ops);
        //0x8F /0 pop rmW
        define_opcode(0x8F).calls(pop).with_gas(VeryLow)
            .is_group(0)
            .with_rmw()
            .into_table(&mut ops);
        //call opcodes
        //0xE8 Call rel16
        //0xE8 Call rel32
        define_opcode(0xE8).calls(call_rel).with_gas(Low)
            .with_arg(ArgSource::JumpRel, NativeWord)
            .is_jump()
            .into_table(&mut ops);
            // need to figure out what this should be
        //0xFF Call r/m16
        //0xFF Call r/m32
        define_opcode(0xFF).is_group(2).calls(call_abs).with_gas(Low)
            .with_rmw()
            .is_unpredictable()
            .into_table(&mut ops);
        //ret opcodes
        //0xC2 RETN
        define_opcode(0xC2).calls(ret)
            .with_imm16()
            .is_unpredictable()
            .into_table(&mut ops);
        //0xC3 RETN
        define_opcode(0xC3).calls(ret)
            .is_unpredictable()
            .into_table(&mut ops);
        //jmp opcodes
        //0xEB  JMP  rel8
        define_opcode(0xEB).calls(jmp_rel).with_gas(Low)
            .with_arg(ArgSource::JumpRel, Fixed(Byte))
            .is_jump()
            .into_table(&mut ops);
        //0xFF /4 JMP  r/mW
        define_opcode(0xFF).is_group(4).calls(jmp_abs).with_gas(Moderate)
            .with_rmw()
            .is_unpredictable()
            .into_table(&mut ops);
        //0xE3 JCXZ rel8
        define_opcode(0xE3).calls(jmp_conditional_ecx_is_zero).with_gas(Low)
            .with_arg(ArgSource::JumpRel, Fixed(Byte))
            .is_unpredictable()
            .into_table(&mut ops);
        //0xE9 JMP  relW
        define_opcode(0xE9).calls(jmp_rel).with_gas(Low)
            .with_arg(ArgSource::JumpRel, NativeWord)
            .is_unpredictable()
            .into_table(&mut ops);
        //0x70-0x7F Jcc rel8
        define_opcode_multi(0x70, 16).calls(jcc).with_gas(Low)
            .with_arg(ArgSource::JumpRel, Fixed(Byte))
            .is_unpredictable()
            .into_table(&mut ops);
        //0x80-0x8F Jcc relw
        define_opcode_multi(0x80, 16).is_two_byte_op().calls(jcc).with_gas(Low)
            .with_arg(ArgSource::JumpRel, NativeWord)
            .is_unpredictable()
            .into_table(&mut ops);

        //Begin maths....
            //sbb opcodes
        //0x18 sbb r/m8, r8
        define_opcode(0x18).calls(sbb_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x19 sbb r/m16, r16
        //0x19 sbb r/m32, r32
        define_opcode(0x19).calls(sbb_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x1A sbb r8, r/m8
        define_opcode(0x1A).calls(sbb_8bit).with_gas(Low)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops);
        //0x1B sbb r16. r/m16
        //0x1B sbb r32, r/m32
        define_opcode(0x1B).calls(sbb_native_word).with_gas(Low)
           .with_rm_regw()
           .with_rmw()
           .into_table(&mut ops);
        //0x1C sbb AL, imm8
        define_opcode(0x1C).calls(sbb_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);
        //0x1D sbb AX, imm16
        //0x1D sbb EAX, imm32
        define_opcode(0x1D).calls(sbb_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 sbb r/m8, imm8
        define_opcode(0x80).is_group(3).calls(sbb_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 sbb r/m16, imm16
        //0x81 sbb r/m32, imm32
        define_opcode(0x81).is_group(3).calls(sbb_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 sbb r/m16, imm8
        //0x83 sbb r/m32, imm8
        define_opcode(0x83).is_group(3).calls(sbb_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
            //adc opcodes
        //0x10 adc r/m8, r8
        define_opcode(0x10).calls(adc_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x11 adc r/m16, r16
        //0x11 adc r/m32, r32
        define_opcode(0x11).calls(adc_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x12 adc r8, r/m8
        define_opcode(0x12).calls(adc_8bit).with_gas(Low)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops);
        //0x13 adc r16. r/m16
        //0x13 adc r32, r/m32
        define_opcode(0x13).calls(adc_native_word).with_gas(Low)
           .with_rm_regw()
           .with_rmw()
           .into_table(&mut ops);
        //0x14 adc AL, imm8
        define_opcode(0x14).calls(adc_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);
        //0x15 adc AX, imm16
        //0x15 adc EAX, imm32
        define_opcode(0x15).calls(adc_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 adc r/m8, imm8
        define_opcode(0x80).is_group(2).calls(adc_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 adc r/m16, imm16
        //0x81 adc r/m32, imm32
        define_opcode(0x81).is_group(2).calls(adc_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 adc r/m16, imm8
        //0x83 adc r/m32, imm8
        define_opcode(0x83).is_group(2).calls(adc_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //add opcodes
        //0x00 add r/m8, r8
        define_opcode(0x00).calls(add_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x01 add r/m16, r16
        //0x01 add r/m32, r32
        define_opcode(0x01).calls(add_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x02 add r8, r/m8
        define_opcode(0x02).calls(add_8bit).with_gas(Low)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops);
        //0x03 add r16. r/m16
        //0x03 add r32, r/m32
        define_opcode(0x03).calls(add_native_word).with_gas(Low)
           .with_rm_regw()
           .with_rmw()
           .into_table(&mut ops);
        //0x04 add AL, imm8
        define_opcode(0x04).calls(add_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);
        //0x05 add AX, imm16
        //0x05 add EAX, imm32
        define_opcode(0x05).calls(add_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 add r/m8, imm8
        define_opcode(0x80).is_group(0).calls(add_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 add r/m16, imm16
        //0x81 add r/m32, imm32
        define_opcode(0x81).is_group(0).calls(add_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 add r/m16, imm8
        //0x83 add r/m32, imm8
        define_opcode(0x83).is_group(0).calls(add_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //sub opcodes
        //0x28 sub r/m8, r8
        define_opcode(0x28).calls(sub_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x29 sub r/m16, r16
        //0x29 sub r/m32, r32
        define_opcode(0x29).calls(sub_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x2A sub r8, r/m8
        define_opcode(0x2A).calls(sub_8bit).with_gas(Low)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops);
        //0x2B sub r16, r/m16
        //0x2B sub r32, r/m32
        define_opcode(0x2B).calls(sub_native_word).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops);
        //0x2C sub AL, imm8
        define_opcode(0x2C).calls(sub_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);
        //0x2D sub AX, imm16
        //0x2D sub EAX, imm32
        define_opcode(0x2D).calls(sub_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 sub r/m8, imm8
        define_opcode(0x80).is_group(5).calls(sub_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 sub r/m16, imm16
        //0x81 sub r/m32, imm32
        define_opcode(0x81).is_group(5).calls(sub_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 sub r/m16, imm8
        //0x83 sub r/m32, imm8
        define_opcode(0x83).is_group(5).calls(sub_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //0xC0 shl r/m8, imm8
        define_opcode(0xC0).is_group(4).calls(shl_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0xC1 shl r/m16, imm8
        //0xC1 shl r/m32, imm8
        define_opcode(0xC1).is_group(4).calls(shl_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //0xD0 shl r/m8, 1
        define_opcode(0xD0).is_group(4).calls(shl_8bit).with_gas(Low)
            .with_rm8()
            .with_arg(ArgSource::Literal(SizedValue::Byte(1)), OpcodeValueSize::Fixed(ValueSize::Byte))
            .into_table(&mut ops);
        //0xD1 shl r/m16, 1
        //0xD1 shl r/m32, 1
        define_opcode(0xD1).is_group(4).calls(shl_native_word).with_gas(Low)
            .with_rmw()
            .with_arg(ArgSource::Literal(SizedValue::Byte(1)), OpcodeValueSize::Fixed(ValueSize::Byte))
            .into_table(&mut ops);
        //0xD2 shl r/m8, CL
        define_opcode(0xD2).is_group(4).calls(shl_8bit).with_gas(Low)
            .with_rm8()
            .with_arg(HardcodedRegister(Reg8::CL as u8), Fixed(Byte))
            .into_table(&mut ops);
        //0xD3 shl r/m16, CL
        //0xD3 shl r/m32, CL
        define_opcode(0xD3).is_group(4).calls(shl_native_word).with_gas(Low)
            .with_rmw()
            .with_arg(HardcodedRegister(Reg8::CL as u8), Fixed(Byte))
            .into_table(&mut ops);
        //0xC0 shr r/m8, imm8
        define_opcode(0xC0).is_group(5).calls(shr_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0xC1 shr r/m16, imm8
        //0xC1 shr r/m32, imm8
        define_opcode(0xC1).is_group(5).calls(shr_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //0xD0 shr r/m8, 1
        define_opcode(0xD0).is_group(5).calls(shr_8bit).with_gas(Low)
            .with_rm8()
            .with_arg(ArgSource::Literal(SizedValue::Byte(1)), OpcodeValueSize::Fixed(ValueSize::Byte))
            .into_table(&mut ops);
        //0xD1 shr r/m16, 1
        //0xD1 shr r/m32, 1
        define_opcode(0xD1).is_group(5).calls(shr_native_word).with_gas(Low)
            .with_rmw()
            .with_arg(ArgSource::Literal(SizedValue::Byte(1)), OpcodeValueSize::Fixed(ValueSize::Byte))
            .into_table(&mut ops);
        //0xD2 shr r/m8, CL
        define_opcode(0xD2).is_group(5).calls(shr_8bit).with_gas(Low)
            .with_rm8()
            .with_arg(HardcodedRegister(Reg8::CL as u8), Fixed(Byte))
            .into_table(&mut ops);
        //0x86 xchg r/m8, r8
        //0x86 xchg r8, r/m8
        define_opcode(0x86).calls(xchg).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x87 xchg r/m16, r16
        //0x87 xchg r16, r/m16
        //0x87 xchg r32, r/m32
        //0x87 xchg r/m32, r32
        define_opcode(0x87).calls(xchg).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x90 xchg ax, r16
        //0x90 xchg r16, ax
        //0x90 xchg eax, r32
        //0x90 xchg r32, eax
        define_opcode(0x90).calls(xchg).with_gas(Low)
            .with_arg(ArgSource::HardcodedRegister(Reg32::EAX as u8), OpcodeValueSize::NativeWord)
            .with_suffix_regw()
            .into_table(&mut ops);
        //0xD3 shr r/m16, CL
        //0xD3 shr r/m32, CL
        define_opcode(0xD3).is_group(5).calls(shr_native_word).with_gas(Low)
            .with_rmw()
            .with_arg(HardcodedRegister(Reg8::CL as u8), Fixed(Byte))
            .into_table(&mut ops);
        //0xF6 mul r/m8
        define_opcode(0xF6).is_group(4).calls(mul_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        //0xF7 mul r/m16
        //0xF7 mul r/m32
        define_opcode(0xF7).is_group(4).calls(mul_native_word).with_gas(Low)
            .with_rmw()
            .into_table(&mut ops);
        //0xF6 imul r/m8
        define_opcode(0xF6).is_group(5).calls(imul1_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        //0xF7 imul r/m16
        //0xF7 imul r/m32
        define_opcode(0xF7).is_group(5).calls(imul1_native_word).with_gas(Low)
            .with_rmw()
            .into_table(&mut ops);
        //0xAF imul r16, r/m16
        //0xAF imul r32, r/m32
        define_opcode(0xAF).is_two_byte_op().calls(imul2_native_word).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops);
        //0x69 imul r16,imm16
        //0x69 imul r16,r/m16,imm16
        //0x69 imul r32,imm32
        //0x69 imul r32,r/m32,imm32
        define_opcode(0x69).calls(imul3_native_word).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x69 imul r16,imm8
        //0x69 imul r16,r/m16,imm8
        //0x69 imul r32,imm8
        //0x69 imul r32,r/m32,imm8
        define_opcode(0x6B).calls(imul3_native_word).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //0xF6 div r/m8
        define_opcode(0xF6).is_group(6).calls(div_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        //0xF7 div r/m16
        //0xF7 div r/m32
        define_opcode(0xF7).is_group(6).calls(div_native_word).with_gas(Low)
            .with_rmw()
            .into_table(&mut ops);
        //0xF6 idiv r/m8
        define_opcode(0xF6).is_group(7).calls(idiv_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        //0xF7 idiv r/m16
        //0xF7 idiv r/m32
        define_opcode(0xF7).is_group(7).calls(idiv_native_word).with_gas(Low)
            .with_rmw()
            .into_table(&mut ops);
        // Begin cmp opcodes
        //0x0F B0 CMPXCHG r/m8, r8
        define_opcode(0xB0).is_two_byte_op().calls(cmpxchg_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x0F B1 CMPXCHG r/m16, r16
        //0x0F B1 CMPXCHG r/m32, r32
        define_opcode(0xB1).is_two_byte_op().calls(cmpxchg_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x38 cmp r/m8, r8
        define_opcode(0x38).calls(cmp_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x39 cmp, r/m16, r16
        //0x39 cmp, r/m32, r32
        define_opcode(0x39).calls(cmp_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x3A cmp r8, r/m8
        define_opcode(0x3A).calls(cmp_8bit).with_gas(Low)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops);
        //0x3B cmp r16, r/m16
        //0x3B cmp r32, r/m32
        define_opcode(0x3B).calls(cmp_native_word).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops);
        //0x3C cmp AL, imm8
        define_opcode(0x3C).calls(cmp_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);        
        //0x3D cmp AX, imm16
        //0x3D cmp EAX, imm32
        define_opcode(0x3D).calls(cmp_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 cmp r/m8, imm8
        define_opcode(0x80).is_group(7).calls(cmp_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 cmp r/m16, imm16
        //0x81 cmp r/m32, imm32
        define_opcode(0x81).is_group(7).calls(cmp_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 cmp r/m16, imm8
        //0x83 cmp r/m32, imm8
        define_opcode(0x83).is_group(7).calls(cmp_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        // Bitwise AND
        //0x20 and r/m8, r8
        define_opcode(0x20).calls(and_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x21 and r/m16, r16
        //0x21 and r/m32, r32
        define_opcode(0x21).calls(and_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x22 and r8, r/m8
        define_opcode(0x22).calls(and_8bit).with_gas(Low)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops);
        //0x23 and r16, r/m16
        //0x23 and r32, r/m32
        define_opcode(0x23).calls(and_native_word).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops);
        //0x24 and AL, imm8
        define_opcode(0x24).calls(and_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);
        //0x25 and AX, imm16
        //0x25 and EAX, imm32
        define_opcode(0x25).calls(and_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 and r/m8, imm8
        define_opcode(0x80).is_group(4).calls(and_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 and r/m16, imm16
        //0x81 and r/m32, imm32
        define_opcode(0x81).is_group(4).calls(and_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 and r/m16, imm8
        //0x83 and r/m32, imm8
        define_opcode(0x83).is_group(4).calls(and_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        // Bitwise OR
        //0x08 or r/m8, r8
        define_opcode(0x08).calls(or_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x09 or r/m16, r16
        //0x09 or r/m32, r32
        define_opcode(0x09).calls(or_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x0A or r8, r/m8
        define_opcode(0x0A).calls(or_8bit).with_gas(Low)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops);
        //0x0B or r16, r/m16
        //0x0B or r32, r/m32
        define_opcode(0x0B).calls(or_native_word).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops);
        //0x0C or AL, imm8
        define_opcode(0x0C).calls(or_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);
        //0x0D or AX, imm16
        //0x0D or EAX, imm32
        define_opcode(0x0D).calls(or_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 or r/m8, imm8
        define_opcode(0x80).is_group(1).calls(or_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 or r/m16, imm16
        //0x81 or r/m32, imm32
        define_opcode(0x81).is_group(1).calls(or_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 or r/m16, imm8
        //0x83 or r/m32, imm8
        define_opcode(0x83).is_group(1).calls(or_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        // Bitwise XOR
        //0x30 xor r/m8, r8
        define_opcode(0x30).calls(xor_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x31 xor r/m16, r16
        //0x31 xor r/m32, r32
        define_opcode(0x31).calls(xor_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x32 xor r8, r/m8
        define_opcode(0x32).calls(xor_8bit).with_gas(Low)
            .with_rm_reg8()
            .with_rm8()
            .into_table(&mut ops);
        //0x33 xor r16, r/m16
        //0x33 xor r32, r/m32
        define_opcode(0x33).calls(xor_native_word).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops);
        //0x34 xor AL, imm8
        define_opcode(0x34).calls(xor_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);
        //0x35 xor AX, imm16
        //0x35 xor EAX, imm32
        define_opcode(0x35).calls(xor_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 xor r/m8, imm8
        define_opcode(0x80).is_group(6).calls(xor_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 xor r/m16, imm16
        //0x81 xor r/m32, imm32
        define_opcode(0x81).is_group(6).calls(xor_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 xor r/m16, imm8
        //0x83 xor r/m32, imm8
        define_opcode(0x83).is_group(6).calls(xor_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
       //Bitwise NOT
        //0xF6 not r/m8,
        define_opcode(0xF6).is_group(2).calls(not_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        //0xF7 not r/m16
        //0xF7 not r/m32
        define_opcode(0xF7).is_group(2).calls(not_native_word).with_gas(Low)
            .with_rmw()
            .into_table(&mut ops);
        // Bitwise NEG
        // 0xF6 neg r/m8
        define_opcode(0xF6).is_group(3).calls(neg_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        //0xF7 neg r/m16
        //0xF7 neg r/m32
        define_opcode(0xF7).is_group(3).calls(neg_native_word).with_gas(Low)
            .with_rmw()
            .into_table(&mut ops);
        // decrement
        // 0x48 dec r16
        // 0x48 dec r32
        define_opcode(0x48).calls(decrement_native_word).with_gas(Low)
            .with_suffix_regw()
            .into_table(&mut ops);
        // 0xFE dec r/m8
        define_opcode(0xFE).is_group(1).calls(decrement_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        // 0xFF dec r/m16
        // 0xFF dec r/m32
        define_opcode(0xFF).is_group(1).calls(decrement_native_word).with_gas(Low)
            .with_rmw()
            .into_table(&mut ops);
        // increment
        // 0x40 inc r16
        // 0x40 inc r32
        define_opcode(0x40).calls(increment_native_word).with_gas(Low)
            .with_suffix_regw()
            .into_table(&mut ops);
        // 0xFE dec r/m8
        define_opcode(0xFE).is_group(0).calls(increment_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        // 0xFF dec r/m16
        // 0xFF dec r/m32
        define_opcode(0xFF).is_group(0).calls(increment_native_word).with_gas(Low)
            .with_rmw()
            .into_table(&mut ops);
        // 0xCD int imm8
        define_opcode(0xCD).calls(interrupt).with_gas(Moderate)
            .with_imm8()
            .is_unpredictable()
            .into_table(&mut ops);
        // 0xCC int3
        define_opcode(0xCC).calls(interrupt).with_gas(Moderate)
            .with_arg(ArgSource::Literal(SizedValue::Byte(3)), OpcodeValueSize::Fixed(ValueSize::Byte))
            .is_unpredictable()
            .into_table(&mut ops);

        //0x0F 90 SETcc rm8
        define_opcode_multi(0x90, 16).is_two_byte_op().calls(setcc_8bit).with_gas(Low)
            .with_rm8()
            .into_table(&mut ops);
        //0x0F 40 CMOVcc rmW
        define_opcode_multi(0x40, 16).is_two_byte_op().calls(cmovcc_native).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops); 
        //0x84 TEST rm8, r8
        define_opcode(0x84).calls(test_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x85 TEST rmW, rW
        define_opcode(0x85).calls(test_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0xA8 TEST AL, imm8
        define_opcode(0xA8).calls(test_8bit).with_gas(Low)
            .with_arg(ArgSource::HardcodedRegister(Reg8::AL as u8), OpcodeValueSize::Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);
        //0xA9 TEST EAX/AX, immW
        define_opcode(0xA9).calls(test_native_word).with_gas(Low)
            .with_arg(ArgSource::HardcodedRegister(Reg32::EAX as u8), OpcodeValueSize::NativeWord)
            .with_immw()
            .into_table(&mut ops);
        //0xF6 /0 TEST rm8, imm8
        define_opcode(0xF6).is_group(0).calls(test_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0xF7 /0 TEST rmW, immW
        define_opcode(0xF7).is_group(0).calls(test_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x8D /r LEA  rW,m
        define_opcode(0x8D).calls(lea).with_gas(Low)
            .with_rm_regw()
            .with_rmw()
            .into_table(&mut ops);
        //0x0F A3 BT r/m16, r16
        //0x0F A3 BT r/m32, r32
        define_opcode(0xA3).is_two_byte_op().calls(bit_test).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0xBA BT r/m16, imm8
        //0xBA BT r/m32, imm8
        define_opcode(0xBA).is_group(4).is_two_byte_op().calls(bit_test).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //0x0F AB BTS r/m16, r16
        //0x0F AB BTS r/m32, r32
        define_opcode(0xAB).is_two_byte_op().calls(bit_test_set).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0xBA BTS r/m16, imm8
        //0xBA BTS r/m32, imm8
        define_opcode(0xBA).is_group(5).is_two_byte_op().calls(bit_test_set).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //0x0F B3 BTR r/m16, r16
        //0x0F B3 BTR r/m32, r32
        define_opcode(0xB3).is_two_byte_op().calls(bit_test_reset).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0xBA BTR r/m16, imm8
        //0xBA BTR r/m32, imm8
        define_opcode(0xBA).is_group(6).is_two_byte_op().calls(bit_test_reset).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //0x0F BB BTC r/m16, r16
        //0x0F BB BTC r/m32, r32
        define_opcode(0xBB).is_two_byte_op().calls(bit_test_complement).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0xBA BTC r/m16, imm8
        //0xBA BTC r/m32, imm8
        define_opcode(0xBA).is_group(7).is_two_byte_op().calls(bit_test_complement).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);
        //0x98 CWDE
        //0x98 CBW
        define_opcode(0x98).calls(cbw_cwde).with_gas(Low)
            .into_table(&mut ops);
        //0x99 CDQ
        //0x99 CWD
        define_opcode(0x99).calls(cdq_cwd).with_gas(Low)
            .into_table(&mut ops);
        //0x0F C8 /r BSWAP r32
        define_opcode(0xC8).is_two_byte_op().calls(bswap).with_gas(Low)
            .with_suffix_reg32()
            .into_table(&mut ops);
        //0x0F BC BSF r16, r/m16
        //0x0F BC BSF r32. r/m32
        define_opcode(0xBC).is_two_byte_op().calls(bit_scan_forward).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x0F BD BSR r16, r/m16
        //0x0F BD BSR r32. r/m32
        define_opcode(0xBD).is_two_byte_op().calls(bit_scan_reverse).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0x0F B6 /r MOVZX rW,rm8
        define_opcode(0xB6).is_two_byte_op().calls(movzx_8bit).with_gas(Low)
            .with_rm_regw()
            .with_rm8()
            .into_table(&mut ops);
        //0x0F B7 /r    MOVZX r32,rm16
        define_opcode(0xB7).is_two_byte_op().calls(movzx_16bit).with_gas(Low)
            .with_rm_reg32()
            .with_rm16()
            .into_table(&mut ops); 
        //0x0F BE /r MOVSX rW,rm8
        define_opcode(0xBE).is_two_byte_op().calls(movsx_8bit).with_gas(Low)
            .with_rm_regw()
            .with_rm8()
            .into_table(&mut ops);
        //0x0F BF /r    MOVSX r32,rm16
        define_opcode(0xBF).is_two_byte_op().calls(movsx_16bit).with_gas(Low)
            .with_rm_reg32()
            .with_rm16()
            .into_table(&mut ops); 
        //0xA5 MOVSD/MOVSW
        define_opcode(0xA5).calls(movs_native_word).with_gas(Low)
            .into_table(&mut ops);
        //0xA4 MOVSB
        define_opcode(0xA4).calls(movsb).with_gas(Low)
            .into_table(&mut ops);
        //0x0F C0 XADD r/m8, r8
        define_opcode(0xC0).is_two_byte_op().calls(xadd_8bit).with_gas(Low)
            .with_rm8()
            .with_rm_reg8()
            .into_table(&mut ops);
        //0x0F C1 XADD r/m32, r32
        define_opcode(0xC1).is_two_byte_op().calls(xadd_native_word).with_gas(Low)
            .with_rmw()
            .with_rm_regw()
            .into_table(&mut ops);
        //0xFD STD
        define_opcode(0xFD).calls(set_direction).with_gas(VeryLow)
            .into_table(&mut ops);
        //0xFC CLD
        define_opcode(0xFC).calls(clear_direction).with_gas(VeryLow)
            .into_table(&mut ops);
        //0xA6 CMPSB
        define_opcode(0xA6).calls(cmpsb).with_gas(Low)
            .into_table(&mut ops);
        //0xA7 CMPSD/CMPSW
        define_opcode(0xA7).calls(cmps_native_word).with_gas(Low)
            .into_table(&mut ops);
        //0xAE SCAS m8
        define_opcode(0xAE).calls(scan_string_byte).with_gas(Low)
            .into_table(&mut ops);
        //0xAE SCAS m16/m32
        define_opcode(0xAF).calls(scan_string_native_word).with_gas(Low)
            .into_table(&mut ops);        
        //0xAA STOS m8
        define_opcode(0xAA).calls(store_string_byte).with_gas(Low)
            .into_table(&mut ops);
        //0xAB STOS m16/m32
        define_opcode(0xAB).calls(store_string_native_word).with_gas(Low)
            .into_table(&mut ops);
        //0xAC LODS m8
        define_opcode(0xAC).calls(load_string_byte).with_gas(Low)
            .into_table(&mut ops);
        //0xAD LODS m16/m32
        define_opcode(0xAD).calls(load_string_native_word).with_gas(Low)
            .into_table(&mut ops);
        //0xC9 LEAVE
        define_opcode(0xC9).calls(leave).with_gas(Low)
            .into_table(&mut ops);
        //0xC8 ENTER imm16, imm8
        define_opcode(0xC8).calls(enter).with_gas(Low)
            .with_imm16()
            .with_imm8()
            .into_table(&mut ops);
        ops
    };
}

