use crate::vm::*;
use crate::pipeline::*;
use crate::structs::*;

/// The logic function for the `mov` opcode
pub fn mov(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    vm.set_arg(pipeline.args[0].location, vm.get_arg(pipeline.args[1].location)?)?;
    Ok(())
}
/// The logic function for the `push` opcode
pub fn push(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let v = vm.get_arg(pipeline.args[0].location)?;
    vm.push_stack(v, pipeline)?;
    Ok(())
}
/// The logic function for the `pop` opcode
pub fn pop(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    //Important edge case:
    /* https://c9x.me/x86/html/file_module_x86_id_248.html
    If the ESP register is used as a base register for addressing a destination operand in memory, 
    the POP instruction computes the effective address of the operand after it increments the ESP register.

    The POP ESP instruction increments the stack pointer (ESP) before data at the old top of stack is written into the destination
    */
    if pipeline.size_override{
        let word = vm.pop16()?;
        vm.set_arg(pipeline.args[0].location, word)?;
    }else{
        let dword = vm.pop32()?;
        vm.set_arg(pipeline.args[0].location, dword)?;
    };
    Ok(())
}

pub fn ret(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError> {
    let stack_clear = vm.get_arg(pipeline.args[0].location)?.u16_zx()?;
    if pipeline.size_override{
        let word = vm.pop16()?;
        vm.eip = (word.u32_zx()? - (pipeline.eip_size as u32)) & 0xFFFF;
    }else{
        let dword = vm.pop32()?;
        vm.eip = dword.u32_zx()? - (pipeline.eip_size as u32);
    };
    if stack_clear != 0 {
        vm.regs[Reg32::ESP as usize] += stack_clear as u32;
    }
    Ok(())
}

pub fn call_rel(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let branch_to = vm.get_arg(pipeline.args[0].location)?.u32_zx()?;
    vm.push_stack(SizedValue::Dword(vm.eip + pipeline.eip_size as u32), pipeline)?;
    vm.set_arg(pipeline.args[1].location, SizedValue::Dword(branch_to))?;
    jmp_rel(vm, pipeline, _hv)?;
    Ok(())
}

pub fn call_abs(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let branch_to = vm.get_arg(pipeline.args[0].location)?.u32_zx()?;
    vm.push_stack(SizedValue::Dword(vm.eip + pipeline.eip_size as u32), pipeline)?;
    vm.set_arg(pipeline.args[1].location, SizedValue::Dword(branch_to))?;
    jmp_abs(vm, pipeline, _hv)?;
    Ok(())
}

/// The logic function for the `jmp` opcodes with a relative argument
pub fn jmp_rel(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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
pub fn jmp_abs(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    //must subtract the size of this opcode to correct for the automatic eip_size advance in the cycle() main loop
    vm.eip = vm.get_arg(pipeline.args[0].location)?.u32_zx()? - (pipeline.eip_size as u32);
    if pipeline.size_override{
        vm.eip &= 0xFFFF;
    }
    Ok(())
}

pub fn jmp_conditional_ecx_is_zero(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.regs[Reg32::ECX as usize] == 0 {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_overflow(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    //check flags for overflow
    if vm.flags.overflow {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_not_overflow(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    //check flags for not overflow
    if vm.flags.overflow == false {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_below(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    //check carry flag
    if vm.flags.carry {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_above_or_equal(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    //check carry not set
    if vm.flags.carry == false {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_is_equal(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.zero{
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_not_equal(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.zero == false {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_below_or_equal(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.carry || vm.flags.zero {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_sign(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.sign {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_no_sign(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.sign == false {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_above(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.carry && vm.flags.zero {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_parity(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.parity{
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_no_parity(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.parity == false {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_less(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.sign != vm.flags.overflow {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_greater_or_equal(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.sign == vm.flags.overflow{
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_less_or_equal(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if (vm.flags.sign != vm.flags.overflow) || vm.flags.zero {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn jmp_greater(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if vm.flags.sign && vm.flags.zero {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn add_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn add_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return add_16bit(vm, pipeline, hv);
    } else {
        return add_32bit(vm, pipeline, hv);
    }
}

/// The logic function for the `hlt` opcode
pub fn hlt(_vm: &mut VM, _pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    Err(VMError::InternalVMStop)
}
pub fn add_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn add_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn increment_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let (result, overflow) = (base as i8).overflowing_add(1 as i8);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result as u8);
    vm.flags.adjust = (base&0x0F) + (1&0x0F) > 15;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn increment_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return increment_16bit(vm, pipeline, hv);
    } else {
        return increment_32bit(vm, pipeline, hv);
    }
}

pub fn increment_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let (result, overflow) = (base as i16).overflowing_add(1 as i16);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result as u16);
    vm.flags.adjust = (base&0x0F) + (1&0x0F) > 15;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn increment_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let (result, overflow) = (base as i32).overflowing_add(1 as i32);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign32(result as u32);
    vm.flags.adjust = (base&0x0F) + (1&0x0F) > 15;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn sub_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn sub_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return sub_16bit(vm, pipeline, hv);
    } else {
        return sub_32bit(vm, pipeline, hv);
    }
}

pub fn sub_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn sub_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn decrement_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError> {
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let (result, overflow) = (base as i8).overflowing_sub(1 as i8);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result as u8);
    vm.flags.adjust = ((base as i32)&0x0F) - ((1 as i32)&0x0F) < 0;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn decrement_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return decrement_16bit(vm, pipeline, hv);
    } else {
        return decrement_32bit(vm, pipeline, hv);
    }
}

pub fn decrement_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let (result, overflow) = (base as i16).overflowing_sub(1 as i16);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result as u16);
    vm.flags.adjust = ((base as i32)&0x0F) - ((1 as i32)&0x0F) < 0;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn decrement_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let (result, overflow) = (base as i32).overflowing_sub(1 as i32);
    vm.flags.overflow = overflow;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign32(result as u32);
    vm.flags.adjust = ((base as i32)&0x0F) - ((1 as i32)&0x0F) < 0;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn cmp_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn cmp_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return cmp_16bit(vm, pipeline, hv);
    } else {
        return cmp_32bit(vm, pipeline, hv);
    }
}

pub fn cmp_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn cmp_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
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

pub fn and_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let result = base & mask;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn and_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return and_16bit(vm, pipeline, hv);
    } else {
        return and_32bit(vm, pipeline, hv);
    }
}

pub fn and_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let result = base & mask;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn and_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let result = base & mask;
    vm.flags.calculate_zero(result);
    vm.flags.calculate_parity(result);
    vm.flags.calculate_sign32(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn or_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let result = base | mask;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn or_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return or_16bit(vm, pipeline, hv);
    } else {
        return or_32bit(vm, pipeline, hv);
    }
}

pub fn or_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let result = base | mask;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn or_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let result = base | mask;
    vm.flags.calculate_zero(result);
    vm.flags.calculate_parity(result);
    vm.flags.calculate_sign32(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn xor_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let result = base ^ mask;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn xor_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return xor_16bit(vm, pipeline, hv);
    } else {
        return xor_32bit(vm, pipeline, hv);
    }
}

pub fn xor_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let result = base ^ mask;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn xor_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let result = base ^ mask;
    vm.flags.calculate_zero(result);
    vm.flags.calculate_parity(result);
    vm.flags.calculate_sign32(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn not_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let result = !base;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn not_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return not_16bit(vm, pipeline, hv);
    } else {
        return not_32bit(vm, pipeline, hv);
    }
}

pub fn not_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let result = !base;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn not_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let result = !base;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn neg_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    vm.flags.carry = base != 0;
    let (result, overflow) = (base as i8).overflowing_neg();
    vm.flags.calculate_zero(result as u32);
    vm.flags.overflow = overflow;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn neg_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return neg_16bit(vm, pipeline, hv);
    } else {
        return neg_32bit(vm, pipeline, hv);
    }
}

pub fn neg_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    vm.flags.carry = base != 0;
    let (result, overflow) = (base as i16).overflowing_neg();
    vm.flags.calculate_zero(result as u32);
    vm.flags.overflow = overflow;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn neg_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    vm.flags.carry = base != 0;
    let (result, overflow) = (base as i32).overflowing_neg();
    vm.flags.calculate_zero(result as u32);
    vm.flags.overflow = overflow;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn interrupt(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError>{
    let num = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    hv.interrupt(vm, num)?;
    Ok(())
}