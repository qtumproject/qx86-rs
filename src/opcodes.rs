use crate::structs::*;
use crate::vm::*;
use crate::pipeline::*;

#[allow(dead_code)] //remove after design stuff is done

pub type OpcodeFn = fn(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>;

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

pub fn nop(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
Ok(())
}

pub fn op_undefined(vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
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
    /// Specifies that the next argument is a ModRM argument of NativeWord size
    pub fn with_rmw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRM, OpcodeValueSize::NativeWord)
    }
    /// Specifies that the next argument is a ModRM /r argument of NativeWord size
    pub fn with_rm_regw(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRMReg, OpcodeValueSize::NativeWord)
    }
    /// Specifies that the next argument is a ModRM /r argument of byte size
    pub fn with_rm_reg8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ModRMReg, OpcodeValueSize::Fixed(ValueSize::Byte))
    }
    /// Specifies that the next argument is an immediate byte value
    pub fn with_imm8(&mut self) -> &mut OpcodeDefiner{
        self.with_arg(ArgSource::ImmediateValue, OpcodeValueSize::Fixed(ValueSize::Byte))
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
        
        //jmp opcodes
        //EB       JMP  rel8
        define_opcode(0xEB).calls(jmp_rel).with_gas(Low)
            .with_arg(ArgSource::JumpRel, Fixed(Byte))
            .is_jump()
            .into_table(&mut ops);
        //FF /4    JMP  r/mW
        define_opcode(0xFF).is_group(4).calls(jmp_abs).with_gas(Moderate)
            .with_rmw()
            .is_unpredictable()
            .into_table(&mut ops);
        //E9       JMP  relW
        define_opcode(0xE9).calls(jmp_rel).with_gas(Low)
            .with_arg(ArgSource::JumpRel, NativeWord)
            .is_jump()
            .into_table(&mut ops);
        
        //Begin maths....
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
        // Begin cmp opcodes
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
        define_opcode(0x3C).calls(sub_8bit).with_gas(Low)
            .with_arg(HardcodedRegister(Reg8::AL as u8), Fixed(Byte))
            .with_imm8()
            .into_table(&mut ops);        
        //0x3D cmp AX, imm16
        //0x3D cmp EAX, imm32
        define_opcode(0x3D).calls(sub_native_word).with_gas(Low)
            .with_arg(HardcodedRegister(Reg32::EAX as u8), NativeWord) //Reg32::EAX resolves to the same as Reg16:AX
            .with_immw()
            .into_table(&mut ops);
        //0x80 cmp r/m8, imm8
        define_opcode(0x80).is_group(7).calls(sub_8bit).with_gas(Low)
            .with_rm8()
            .with_imm8()
            .into_table(&mut ops);
        //0x81 cmp r/m16, imm16
        //0x81 cmp r/m32, imm32
        define_opcode(0x81).is_group(7).calls(sub_native_word).with_gas(Low)
            .with_rmw()
            .with_immw()
            .into_table(&mut ops);
        //0x83 cmp r/m16, imm8
        //0x83 cmp r/m32, imm8
        define_opcode(0x83).is_group(7).calls(sub_native_word).with_gas(Low)
            .with_rmw()
            .with_imm8()
            .into_table(&mut ops);        
        ops
    };
}

