use crate::opcodes::*;
use crate::vm::*;
use crate::pipeline::*;
use crate::structs::*;


pub fn mov(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    vm.set_arg(pipeline.args[0].location, vm.get_arg(pipeline.args[1].location)?)?;
    Ok(())
}

pub fn push(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let v = vm.get_arg(pipeline.args[0].location)?;
    if pipeline.size_override{
        vm.regs[Reg32::ESP as usize] -= 2;
        vm.set_mem(vm.regs[Reg32::ESP as usize], SizedValue::Word(v.u16_zx()?))?;
    }else{
        vm.regs[Reg32::ESP as usize] -= 4;
        vm.set_mem(vm.regs[Reg32::ESP as usize], SizedValue::Dword(v.u32_zx()?))?;
    };
    Ok(())
}
pub fn pop(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    //Important edge case
    /* https://c9x.me/x86/html/file_module_x86_id_248.html
    If the ESP register is used as a base register for addressing a destination operand in memory, 
    the POP instruction computes the effective address of the operand after it increments the ESP register.

    The POP ESP instruction increments the stack pointer (ESP) before data at the old top of stack is written into the destination
    */
    let esp = vm.regs[Reg32::ESP as usize];
    if pipeline.size_override{
        vm.regs[Reg32::ESP as usize] += 2;
        vm.set_arg(pipeline.args[0].location, vm.get_mem(esp, ValueSize::Word)?)?;
    }else{
        vm.regs[Reg32::ESP as usize] += 4;
        vm.set_arg(pipeline.args[0].location, vm.get_mem(esp, ValueSize::Dword)?)?;
    };
    Ok(())
}


pub fn hlt(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    Err(VMError::InternalVMStop)
}
