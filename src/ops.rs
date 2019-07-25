use crate::vm::*;
use crate::pipeline::*;
use crate::structs::*;

/// The logic function for the `mov` opcode
pub fn mov(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    vm.set_arg(pipeline.args[0].location, vm.get_arg(pipeline.args[1].location)?)?;
    Ok(())
}
/// The logic function for the `push` opcode
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
/// The logic function for the `pop` opcode
pub fn pop(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    //Important edge case:
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
/// The logic function for the `jmp` opcodes with a relative argument
pub fn jmp_rel(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    //relative jumps are calculated from the EIP value AFTER the jump would've executed, ie, after EIP is advanced by the size of the instruction
    let future_eip = vm.eip + (pipeline.eip_size as u32);
    //rel must be sign extended, but is otherwise treated as a u32 for simplicity
    //an i32 and a u32 will behave the same way for wrapping_addition like this
    let rel = vm.get_arg(pipeline.args[0].location)?.u32_sx()?;
    //subtract out the eip_size that'll be advanced in the cycle() main loop
    vm.eip = future_eip.wrapping_add(rel) - (pipeline.eip_size as u32);
    if pipeline.size_override{
        vm.eip &= 0xFFFF;
    }
    Ok(())
}
/// The logic function for the `jmp` opcodes with an absolute argument
pub fn jmp_abs(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    //must subtract the size of this opcode to correct for the automatic eip_size advance in the cycle() main loop
    vm.eip = vm.get_arg(pipeline.args[0].location)?.u32_zx()? - (pipeline.eip_size as u32);
    if pipeline.size_override{
        vm.eip &= 0xFFFF;
    }
    Ok(())
}



/// The logic function for the `hlt` opcode
pub fn hlt(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
    Err(VMError::InternalVMStop)
}
