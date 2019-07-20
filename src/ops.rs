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

pub fn add_8bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let adder = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let (result, overflow) = base.overflowing_add(adder);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
    vm.flags.carry = result < std::cmp::min(base, adder);
    vm.flags.adjust = (base&0x0F) + (adder&0x0F) > 15;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result))?;
    Ok(())
}

pub fn add_native_word(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError> {
    if pipeline.size_override {
        return add_16bit(vm, pipeline);
    } else {
        return add_32bit(vm, pipeline);
    }
}

pub fn add_16bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let adder = vm.get_arg(pipeline.args[1].location)?.u16_exact()?;
    let (result, overflow) = base.overflowing_add(adder);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result);
    vm.flags.carry = result < std::cmp::min(base, adder);
    vm.flags.adjust = (base&0x0F) + (adder&0x0F) > 15;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result))?;
    Ok(())
}

pub fn add_32bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let adder = vm.get_arg(pipeline.args[1].location)?.u32_exact()?;
    let (result, overflow) = base.overflowing_add(adder);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result);
    vm.flags.calculate_parity(result);
    vm.flags.calculate_sign32(result);
    vm.flags.carry = result < std::cmp::min(base, adder);
    vm.flags.adjust = (base&0x0F) + (adder&0x0F) > 15;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result))?;
    Ok(())
}

pub fn hlt(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
    Err(VMError::InternalVMStop)
}
