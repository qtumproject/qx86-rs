use crate::opcodes::*;
use crate::structs::*;
use crate::vm::*;
use crate::decoding::*;

#[allow(dead_code)] //remove after design stuff is done


pub struct Pipeline{
    pub function: OpcodeFn,
    pub args: [OpArgument; MAX_ARGS],
    pub gas_cost: i32,
    pub eip_size: u8
}

impl Default for Pipeline{
    fn default() -> Pipeline {
        Pipeline{
            function: nop,
            args: [OpArgument::default(), OpArgument::default(), OpArgument::default()],
            gas_cost: 0,
            eip_size: 1
        }
    }
}


fn fill_pipeline(vm: &VM, pipeline: &mut [Pipeline]) -> Result<(), VMError>{
    let mut eip = vm.eip;
    let mut stop_filling = false;
    for n in 0..pipeline.len(){
        let mut p = &mut pipeline[n];
        if stop_filling {
            p.function = nop;
            p.args = [OpArgument::default(); 3];
            p.eip_size = 0;
            p.gas_cost = 0;
        }else{
            let buffer = vm.memory.get_sized_memory(eip, 16)?;
            //todo: handle the upper bits of opcode
            let opcode = &OPCODES[buffer[0 as usize] as usize];
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

    }
    Ok(())
}





