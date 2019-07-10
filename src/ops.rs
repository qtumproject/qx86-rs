use crate::opcodes::*;
use crate::vm::*;
use crate::pipeline::*;


pub fn mov(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    vm.set_arg(pipeline.args[0].location, vm.get_arg(pipeline.args[1].location)?)?;
    Ok(())
}

pub fn hlt(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    Err(VMError::InternalVMStop)
}
