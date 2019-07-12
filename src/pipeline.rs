use crate::opcodes::*;
use crate::structs::*;
use crate::vm::*;
use crate::decoding::*;

#[allow(dead_code)] //remove after design stuff is done

#[derive(Copy, Clone)]
pub struct Pipeline{
    pub function: OpcodeFn,
    pub args: [OpArgument; MAX_ARGS],
    pub gas_cost: i32,
    pub eip_size: u8,
    pub size_override: bool
}

impl Default for Pipeline{
    fn default() -> Pipeline {
        Pipeline{
            function: nop,
            args: [OpArgument::default(), OpArgument::default(), OpArgument::default()],
            gas_cost: 0,
            eip_size: 1,
            size_override: false
        }
    }
}

pub fn clear_pipeline(pipeline: &mut [Pipeline]){
    for _n in 0..pipeline.len(){
        pipeline[0] = Pipeline::default();
    }
}

pub fn fill_pipeline(vm: &VM, opcodes: &[Opcode], pipeline: &mut [Pipeline]) -> Result<(), VMError>{
    let mut eip = vm.eip;
    let mut stop_filling = false;
    //writeable if in memory with top bit set
    let writeable = vm.eip & 0x8000000 > 0;
    clear_pipeline(pipeline);
    for n in 0..pipeline.len(){
        let mut p = &mut pipeline[n];
        if stop_filling {
            p.function = nop;
            p.eip_size = 0;
            p.gas_cost = 0;
        }else{
            let buffer = vm.memory.get_sized_memory(eip, 16)?;
            //todo: handle the upper bits of opcode
            let opcode = &opcodes[buffer[0 as usize] as usize];
            match opcode.jump_behavior{
                JumpBehavior::None => {
                    p.function = opcode.function;
                    p.gas_cost = opcode.gas_cost;
                    p.eip_size = decode_args(opcode, buffer, &mut p.args, false)? as u8;
                },
                JumpBehavior::Conditional => {
                    p.function = opcode.function;
                    p.gas_cost = opcode.gas_cost;
                    p.eip_size = decode_args(opcode, buffer, &mut p.args, false)? as u8;
                    eip += p.eip_size as u32;
                    stop_filling = true;
                },
                JumpBehavior::Relative => {
                    //todo: later follow jumps that can be predicted
                    //right now this is just copy-pasted from conditional jumps
                    p.function = opcode.function;
                    p.gas_cost = opcode.gas_cost;
                    p.eip_size = decode_args(opcode, buffer, &mut p.args, false)? as u8;
                    eip += p.eip_size as u32;
                    stop_filling = true;
                }
            };
            eip += p.eip_size as u32;
        }
        if writeable {
            //if in writeable space, only use one pipeline slot at a time
            //otherwise, the memory we are decoding could be changed by an opcode within the pipeline
            stop_filling = true;
        }

    }
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;

    //just a simple test function for comparison
    fn test_op(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{Ok(())}
    fn test2_op(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{Ok(())}
    fn test3_op(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{Ok(())}

    /* Opcodes defined:
    0x00 -- undefined (purposefully)
    0x01 (), 10 gas -- test_op
    0x02 (imm8), 2 gas -- nop
    0x03 (imm32), 50 gas -- test3_op, conditional jump behavior
    0x10 + r (reg32, off32), 23 gas -- test2_op
    */
    fn test_opcodes() -> Vec<Opcode>{
        let mut opcodes = vec![];
        opcodes.resize(OPCODE_TABLE_SIZE, Opcode::default());
        opcodes[0x01].gas_cost = 10;
        opcodes[0x01].function = test_op;
        
        opcodes[0x02].arg_source[0] = ArgSource::ImmediateValue;
        opcodes[0x02].arg_size[0] = ValueSize::Byte;
        opcodes[0x02].gas_cost = 2;
        opcodes[0x02].function = nop;
        
        opcodes[0x03].arg_source[0] = ArgSource::JumpRel;
        opcodes[0x03].arg_size[0] = ValueSize::Dword;
        opcodes[0x03].jump_behavior = JumpBehavior::Conditional;
        opcodes[0x03].function = test3_op;
        opcodes[0x03].gas_cost = 50;

        for n in 0..8{
            opcodes[0x10 + n].arg_source[0] = ArgSource::RegisterSuffix;
            opcodes[0x10 + n].arg_source[1] = ArgSource::ImmediateAddress;
            opcodes[0x10 + n].arg_size[0] = ValueSize::Dword;
            opcodes[0x10 + n].arg_size[1] = ValueSize::Dword;
            opcodes[0x10 + n].gas_cost = 23;
            opcodes[0x10 + n].function = test2_op;
        }

        opcodes
    }

    #[test]
    fn test_simple_pipeline(){
        let opcodes = test_opcodes();
        let mut vm = VM::default();
        let vm_mem = vm.memory.add_memory(0x10000, 0x100).unwrap();
        vm.eip = 0x10000;
        let bytes = vec![
            0x01, //test_op
            0x02, 0x15, //nop, imm8
            0x12, 0x11, 0x22, 0x33, 0x44, //test2_op, EDX, off32
        ];
        (&mut vm_mem[0..bytes.len()]).copy_from_slice(&bytes);
        let mut pipeline = vec![];
        pipeline.resize(2, Pipeline::default());
        fill_pipeline(&vm, &opcodes, &mut pipeline).unwrap();
        
        //Function pointers must be cast to usize here
        //This may break later, so do NOT do in production code
        //See also: https://www.reddit.com/r/rust/comments/98xlh3/how_can_i_compare_two_function_pointers_to_see_if/
        //If weird things break here later, it might be worth figuring out if this is the reason

        assert!(pipeline[0].function as usize == test_op as usize);
        assert!(pipeline[0].args[0].location == ArgLocation::None);
        assert!(pipeline[0].eip_size == 1);

        assert!(pipeline[1].function as usize == nop as usize);
        assert!(pipeline[1].args[0].location == ArgLocation::Immediate(SizedValue::Byte(0x15)));
        assert!(pipeline[1].args[1].location == ArgLocation::None);
        assert!(pipeline[1].eip_size == 2);

        vm.eip += pipeline[0].eip_size as u32 + pipeline[1].eip_size as u32;
        fill_pipeline(&vm, &opcodes, &mut pipeline).unwrap();

        assert!(pipeline[0].function as usize == test2_op as usize);
        assert!(pipeline[0].args[0].location == ArgLocation::RegisterValue(2, ValueSize::Dword));
        assert!(pipeline[0].args[1].location == ArgLocation::Address(0x44332211, ValueSize::Dword));
        assert!(pipeline[0].eip_size == 5); 
    }

    #[test]
    fn test_cond_jump_pipeline(){
        let opcodes = test_opcodes();
        let mut vm = VM::default();
        let vm_mem = vm.memory.add_memory(0x10000, 0x100).unwrap();
        vm.eip = 0x10000;
        let bytes = vec![
            0x01, //test_op
            0x03, 0x11, 0x22, 0x33, 0x44, //test3_op, imm32, cond jump
            0x12, 0x11, 0x22, 0x33, 0x44, //test2_op, EDX, off32
        ];
        (&mut vm_mem[0..bytes.len()]).copy_from_slice(&bytes);
        let mut pipeline = vec![];
        pipeline.resize(3, Pipeline::default());
        fill_pipeline(&vm, &opcodes, &mut pipeline).unwrap();

        assert!(pipeline[0].function as usize == test_op as usize);
        assert!(pipeline[0].args[0].location == ArgLocation::None);
        assert!(pipeline[0].eip_size == 1);

        assert!(pipeline[1].function as usize == test3_op as usize);
        assert!(pipeline[1].args[0].location == ArgLocation::Immediate(SizedValue::Dword(0x44332211)));
        assert!(pipeline[1].args[1].location == ArgLocation::None);
        assert!(pipeline[1].eip_size == 5); 

        //ensure next opcode after conditional jump is nop
        assert!(pipeline[2].function as usize == nop as usize);
        assert!(pipeline[2].args[0].location == ArgLocation::None);
        assert!(pipeline[2].args[1].location == ArgLocation::None);
        assert!(pipeline[2].eip_size == 0);  
        assert!(pipeline[2].gas_cost == 0);  
    }
}



