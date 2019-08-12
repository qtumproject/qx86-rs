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

pub fn add_8bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let adder = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let (result, carry) = base.overflowing_add(adder);
    let (_, overflow) = (base as i8).overflowing_add(adder as i8);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
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

/// The logic function for the `hlt` opcode
pub fn hlt(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{
    Err(VMError::InternalVMStop)
}
pub fn add_16bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let adder = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let (result, carry) = base.overflowing_add(adder);
    let (_, overflow) = (base as i16).overflowing_add(adder as i16);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result);
    vm.flags.adjust = (base&0x0F) + (adder&0x0F) > 15;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result))?;
    Ok(())
}

pub fn add_32bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let adder = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let (result, carry) = base.overflowing_add(adder);
    let (_, overflow) = (base as i32).overflowing_add(adder as i32);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result);
    vm.flags.calculate_parity(result);
    vm.flags.calculate_sign32(result);
    vm.flags.adjust = (base&0x0F) + (adder&0x0F) > 15;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result))?;
    Ok(())
}

pub fn sub_8bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let subt = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let (result, carry) = base.overflowing_sub(subt);
    let (_, overflow) = (base as i8).overflowing_sub(subt as i8);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
    vm.flags.adjust = ((base as i32)&0x0F) - ((subt as i32)&0x0F) < 0;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result))?;
    Ok(())
}

pub fn sub_native_word(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError> {
    if pipeline.size_override {
        return sub_16bit(vm, pipeline);
    } else {
        return sub_32bit(vm, pipeline);
    }
}

pub fn sub_16bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let subt = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let (result, carry) = base.overflowing_sub(subt);
    let (_, overflow) = (base as i16).overflowing_sub(subt as i16);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result);
    vm.flags.adjust = ((base as i32)&0x0F) - ((subt as i32)&0x0F) < 0;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result))?;
    Ok(())
}

pub fn sub_32bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let subt = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let (result, carry) = base.overflowing_sub(subt);
    let (_, overflow) = (base as i32).overflowing_sub(subt as i32);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result);
    vm.flags.calculate_parity(result);
    vm.flags.calculate_sign32(result);
    vm.flags.adjust = ((base as i32)&0x0F) - ((subt as i32)&0x0F) < 0;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result))?;
    Ok(())
}

pub fn cmp_8bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let cmpt = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let (result, carry) = base.overflowing_sub(cmpt);
    let (_, overflow) = (base as i8).overflowing_sub(cmpt as i8);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
    vm.flags.adjust = ((base as i32)&0x0F) - ((cmpt as i32)&0x0F) < 0;
    Ok(())
}

pub fn cmp_native_word(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError> {
    if pipeline.size_override {
        return cmp_16bit(vm, pipeline);
    } else {
        return cmp_32bit(vm, pipeline);
    }
}

pub fn cmp_16bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let cmpt = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let (result, carry) = base.overflowing_sub(cmpt);
    let (_, overflow) = (base as i16).overflowing_sub(cmpt as i16);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result);
    vm.flags.adjust = ((base as i32)&0x0F) - ((cmpt as i32)&0x0F) < 0;
    Ok(())
}

pub fn cmp_32bit(vm: &mut VM, pipeline: &Pipeline) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let cmpt = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let (result, carry) = base.overflowing_sub(cmpt);
    let (_, overflow) = (base as i32).overflowing_sub(cmpt as i32);
    vm.flags.overflow = overflow;
    vm.flags.carry = carry;
    vm.flags.calculate_zero(result);
    vm.flags.calculate_parity(result);
    vm.flags.calculate_sign32(result);
    vm.flags.adjust = ((base as i32)&0x0F) - ((cmpt as i32)&0x0F) < 0;
    Ok(())
}