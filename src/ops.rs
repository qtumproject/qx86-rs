use crate::vm::*;
use crate::pipeline::*;
use crate::structs::*;
use crate::flags::X86Flags;

/// The logic function for the `mov` opcode
pub fn mov(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    vm.set_arg(pipeline.args[0].location, vm.get_arg(pipeline.args[1].location)?)?;
    Ok(())
}
/// The logic function for the `push` opcode
pub fn push(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let v = vm.get_arg(pipeline.args[0].location)?;
    vm.push_stack(v, pipeline)?;
    Ok(())
}
/// The logic function for the `pop` opcode
pub fn pop(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn xchg(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let source = vm.get_arg(pipeline.args[0].location)?;
    let destination = vm.get_arg(pipeline.args[1].location)?;
    vm.set_arg(pipeline.args[0].location, destination)?;
    vm.set_arg(pipeline.args[1].location, source)?;
    Ok(())
}

pub fn ret(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
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

pub fn call_rel(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let branch_to = vm.get_arg(pipeline.args[0].location)?.u32_zx()?;
    vm.push_stack(SizedValue::Dword(vm.eip + pipeline.eip_size as u32), pipeline)?;
    vm.set_arg(pipeline.args[1].location, SizedValue::Dword(branch_to))?;
    jmp_rel(vm, pipeline, _hv)?;
    Ok(())
}

pub fn call_abs(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let branch_to = vm.get_arg(pipeline.args[0].location)?.u32_zx()?;
    vm.push_stack(SizedValue::Dword(vm.eip + pipeline.eip_size as u32), pipeline)?;
    vm.set_arg(pipeline.args[1].location, SizedValue::Dword(branch_to))?;
    jmp_abs(vm, pipeline, _hv)?;
    Ok(())
}

/// The logic function for the `jmp` opcodes with a relative argument
pub fn jmp_rel(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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
pub fn jmp_abs(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    //must subtract the size of this opcode to correct for the automatic eip_size advance in the cycle() main loop
    vm.eip = vm.get_arg(pipeline.args[0].location)?.u32_zx()? - (pipeline.eip_size as u32);
    if pipeline.size_override{
        vm.eip &= 0xFFFF;
    }
    Ok(())
}

pub fn jmp_conditional_ecx_is_zero(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if vm.regs[Reg32::ECX as usize] == 0 {
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

fn cc_matches(opcode: u8, flags: &X86Flags) -> bool{
    let cc = 0x0F & opcode;
    match cc{
        0x0 => flags.overflow,
        0x1 => !flags.overflow,
        0x2 => flags.carry,
        0x3 => !flags.carry,
        0x4 => flags.zero,
        0x5 => !flags.zero,
        0x6 => flags.carry | flags.zero,
        0x7 => !flags.carry & !flags.zero,
        0x8 => flags.sign,
        0x9 => !flags.sign,
        0xA => flags.parity,
        0xB => !flags.parity,
        0xC => flags.sign != flags.overflow,
        0xD => flags.sign == flags.overflow,
        0xE => (flags.sign != flags.overflow) | flags.zero,
        0xF => (flags.sign == flags.overflow) & !flags.zero,
        _ => false
    }
}

pub fn jcc(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if cc_matches(pipeline.opcode, &vm.flags){
        return jmp_rel(vm, pipeline, hv);
    }
    Ok(())
}

pub fn div_8bit(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.reg16(Reg16::AX) as u16;
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u16_zx()?;
    if second_arg == 0 || ((first_arg / second_arg) > 0xFF) {
        return Err(VMError::DivideByZero) // divide by 0 not allowed and result being too big for destination not allowed
    }
    vm.set_reg(Reg8::AL as u8, SizedValue::Byte((first_arg / second_arg) as u8));
    vm.set_reg(Reg8::AH as u8, SizedValue::Byte((first_arg%second_arg) as u8));
    Ok(())
}

pub fn div_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return div_16bit(vm, pipeline, hv);
    } else {
        return div_32bit(vm, pipeline, hv);
    }
}

pub fn div_16bit(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    let first_arg = ((vm.reg16(Reg16::DX) as u32) << 16) | (vm.reg16(Reg16::AX) as u32);
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u32_zx()?;
    if second_arg == 0 || ((first_arg / second_arg) > 0xFFFF) {
        return Err(VMError::DivideByZero) // divide by 0 not allowed and result being too big for destination not allowed
    }
    vm.set_reg(Reg16::AX as u8, SizedValue::Word((first_arg / second_arg) as u16));
    vm.set_reg(Reg16::DX as u8, SizedValue::Word((first_arg%second_arg) as u16));
    Ok(())
}

pub fn div_32bit(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    let first_arg = ((vm.reg32(Reg32::EDX) as u64) << 32) | (vm.reg32(Reg32::EAX) as u64);
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u32_zx()? as u64;
    if second_arg == 0 || ((first_arg / second_arg) > 0xFFFFFFFF) {
        return Err(VMError::DivideByZero) // divide by 0 not allowed and result being too big for destination not allowed
    }
    vm.set_reg(Reg32::EAX as u8, SizedValue::Dword((first_arg / second_arg) as u32));
    vm.set_reg(Reg32::EDX as u8, SizedValue::Dword((first_arg%second_arg) as u32));
    Ok(())
}

pub fn idiv_8bit(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.reg16(Reg16::AX) as i16;
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u16_zx()? as i16;
    if second_arg == 0 || ((first_arg / second_arg) > 0xFF) {
        return Err(VMError::DivideByZero) // divide by 0 not allowed and result being too big for destination not allowed
    }
    vm.set_reg(Reg8::AL as u8, SizedValue::Byte((first_arg / second_arg) as u8));
    vm.set_reg(Reg8::AH as u8, SizedValue::Byte((first_arg%second_arg) as u8));
    Ok(())
}

pub fn idiv_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return idiv_16bit(vm, pipeline, hv);
    } else {
        return idiv_32bit(vm, pipeline, hv);
    }
}

pub fn idiv_16bit(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    let first_arg = ((vm.reg16(Reg16::DX) as i16 as i32) << 16) | (vm.reg16(Reg16::AX) as u16 as i32);
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u32_zx()? as i32;
    if second_arg == 0 || ((first_arg / second_arg) > 0xFFFF) {
        return Err(VMError::DivideByZero) // divide by 0 not allowed and result being too big for destination not allowed
    }
    vm.set_reg(Reg16::AX as u8, SizedValue::Word((first_arg / second_arg) as u16));
    vm.set_reg(Reg16::DX as u8, SizedValue::Word((first_arg%second_arg) as u16));
    Ok(())
}

pub fn idiv_32bit(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    let first_arg = ((vm.reg32(Reg32::EDX) as i32 as i64) << 32) | (vm.reg32(Reg32::EAX) as i32 as i64);
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u32_zx()? as i32 as i64;
    if second_arg == 0 || ((first_arg / second_arg) > 0xFFFFFFFF) {
        return Err(VMError::DivideByZero) // divide by 0 not allowed and result being too big for destination not allowed
    }
    vm.set_reg(Reg32::EAX as u8, SizedValue::Dword((first_arg / second_arg) as u32));
    vm.set_reg(Reg32::EDX as u8, SizedValue::Dword((first_arg%second_arg) as u32));
    Ok(())
}

pub fn mul_8bit(vm: &mut VM, pipeline: &Pipeline, hv: &mut Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.reg8(Reg8::AL) as u16;
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u16_zx()?;
    let result = first_arg.wrapping_mul(second_arg);
    if result & 0xFF00 > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    vm.set_reg(Reg16::AX as u8, SizedValue::Word(result));
    Ok(())
}

pub fn mul_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return mul_16bit(vm, pipeline, hv);
    } else {
        return mul_32bit(vm, pipeline, hv);
    }
}

pub fn mul_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.reg16(Reg16::AX) as u32;
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u32_zx()?;
    let result = first_arg.wrapping_mul(second_arg);
    vm.set_reg(Reg16::AX as u8, SizedValue::Word((result&0x0000FFFF) as u16));
    vm.set_reg(Reg16::DX as u8, SizedValue::Word(((result&0xFFFF0000).wrapping_shr(16)) as u16));
    if vm.reg16(Reg16::DX) > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    Ok(())
}

pub fn mul_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.reg32(Reg32::EAX) as u64;
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u32_zx()? as u64;
    let result = first_arg.wrapping_mul(second_arg);
    vm.set_reg(Reg32::EAX as u8, SizedValue::Dword((result&0x00000000FFFFFFFF) as u32));
    vm.set_reg(Reg32::EDX as u8, SizedValue::Dword(((result&0xFFFFFFFF00000000).wrapping_shr(32)) as u32));
    if vm.reg32(Reg32::EDX) > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    Ok(())
}

pub fn imul1_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.reg8(Reg8::AL) as i16;
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u16_sx()? as i16;
    let result = (first_arg.wrapping_mul(second_arg)) as u16;
    if result & 0xFF00 > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    vm.set_reg(Reg16::AX as u8, SizedValue::Word(result));
    Ok(())
}

pub fn imul1_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return imul1_16bit(vm, pipeline, hv);
    } else {
        return imul1_32bit(vm, pipeline, hv);
    }
}

pub fn imul1_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.reg16(Reg16::AX) as i16 as i32;
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u16_sx()? as i16 as i32;
    let result = (first_arg.wrapping_mul(second_arg)) as u32;
    vm.set_reg(Reg16::AX as u8, SizedValue::Word((result&0x0000FFFF) as u16));
    vm.set_reg(Reg16::DX as u8, SizedValue::Word(((result&0xFFFF0000).wrapping_shr(16)) as u16));
    if vm.reg16(Reg16::DX) > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    Ok(())
}

pub fn imul1_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.reg32(Reg32::EAX) as i32 as i64;
    let second_arg = vm.get_arg(pipeline.args[0].location)?.u32_sx()? as i32 as i64;
    let result = (first_arg.wrapping_mul(second_arg)) as u64;
    vm.set_reg(Reg32::EAX as u8, SizedValue::Dword((result&0x00000000FFFFFFFF) as u32));
    vm.set_reg(Reg32::EDX as u8, SizedValue::Dword(((result&0xFFFFFFFF00000000).wrapping_shr(32)) as u32));
    if vm.reg32(Reg32::EDX) > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    Ok(())
}

pub fn imul2_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return imul2_16bit(vm, pipeline, hv);
    } else {
        return imul2_32bit(vm, pipeline, hv);
    }
}

pub fn imul2_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.get_arg(pipeline.args[0].location)?.u16_sx()? as i16 as i32;
    let second_arg = vm.get_arg(pipeline.args[1].location)?.u16_sx()? as i16 as i32;
    let result = (first_arg.wrapping_mul(second_arg)) as u32;
    if (result&0xFFFF0000).wrapping_shr(16) > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    vm.set_arg(pipeline.args[0].location, SizedValue::Word((result&0x0000FFFF) as u16))?;
    Ok(())
}

pub fn imul2_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.get_arg(pipeline.args[0].location)?.u32_sx()? as i32 as i64;
    let second_arg = vm.get_arg(pipeline.args[1].location)?.u32_sx()? as i32 as i64;
    let result = (first_arg.wrapping_mul(second_arg)) as u64;
    if (result&0xFFFFFFFF00000000).wrapping_shr(16) > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword((result&0x00000000FFFFFFFF) as u32))?;
    Ok(())
}

pub fn imul3_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return imul3_16bit(vm, pipeline, hv);
    } else {
        return imul3_32bit(vm, pipeline, hv);
    }
}

pub fn imul3_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.get_arg(pipeline.args[1].location)?.u32_sx()? as i32 as i64;;
    let second_arg = vm.get_arg(pipeline.args[2].location)?.u32_sx()? as i32 as i64;;
    let result = (first_arg.wrapping_mul(second_arg)) as u32;
    if (result&0xFFFF0000).wrapping_shr(16) > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    vm.set_arg(pipeline.args[0].location, SizedValue::Word((result&0x0000FFFF) as u16))?;
    Ok(())
}

pub fn imul3_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    let first_arg = vm.get_arg(pipeline.args[1].location)?.u32_sx()? as i32 as i64;
    let second_arg = vm.get_arg(pipeline.args[2].location)?.u32_sx()? as i32 as i64;
    let result = (first_arg.wrapping_mul(second_arg)) as u64;
    if (result&0xFFFFFFFF00000000).wrapping_shr(16) > 0 {
        vm.flags.carry = true;
        vm.flags.overflow = true;
    } else {
        vm.flags.carry = false;
        vm.flags.overflow = false;
    }
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword((result&0x00000000FFFFFFFF) as u32))?;
    Ok(())
}

pub fn shl_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let destination = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let count = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let result= (destination as u16) << count;
    if count == 1 {
        vm.flags.overflow = (destination & 0x80) != ((result as u8) & 0x80);
    }
    vm.flags.carry = result & 0x100 != 0;
    vm.flags.calculate_zero(result as u8 as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result as u8);
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn shl_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return shl_16bit(vm, pipeline, hv);
    } else {
        return shl_32bit(vm, pipeline, hv);
    }
}

pub fn shl_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let destination = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let count = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let result= (destination as u32) << count;
    if count == 1 {
        vm.flags.overflow = ((destination as u32) & 0x8000) != (result & 0x8000);
    }
    vm.flags.carry = (result & 0x1000) != 0;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result as u16);
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn shl_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let destination = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let count = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let result= (destination as u64) << count;
    if count == 1 {
        vm.flags.overflow = ((destination as u64) & 0x80000000) != (result & 0x80000000);
    }
    vm.flags.carry = (result & 0x100000000) != 0;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign32(result as u32);
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn shr_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let destination = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let count = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let result= destination >> count;
    let computation_result = destination >> (count - 1);
    if count == 1 {
        vm.flags.overflow = (destination & 0x80) != (result & 0x80);
    }
    vm.flags.carry = computation_result & 1 != 0;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn shr_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return shr_16bit(vm, pipeline, hv);
    } else {
        return shr_32bit(vm, pipeline, hv);
    }
}

pub fn shr_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let destination = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let count = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let result= (destination as u32) >> count;
    let computation_result = (destination as u32) >> (count - 1);
    if count == 1 {
        vm.flags.overflow = ((destination as u32) & 0x8000) != (result & 0x8000);
    }
    vm.flags.carry = (computation_result & 1) != 0;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result as u16);
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn shr_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let destination = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let count = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let result= (destination as u64) >> count;
    let computation_result = destination >> (count - 1);
    if count == 1 {
        vm.flags.overflow = ((destination as u64) & 0x80000000) != (result & 0x80000000);
    }
    vm.flags.carry = (computation_result & 1) != 0;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign32(result as u32);
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn add_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn add_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return add_16bit(vm, pipeline, hv);
    } else {
        return add_32bit(vm, pipeline, hv);
    }
}

/// The logic function for the `hlt` opcode
pub fn hlt(_vm: &mut VM, _pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    Err(VMError::InternalVMStop)
}
pub fn add_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn add_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn increment_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn increment_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return increment_16bit(vm, pipeline, hv);
    } else {
        return increment_32bit(vm, pipeline, hv);
    }
}

pub fn increment_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn increment_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn sub_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn sub_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return sub_16bit(vm, pipeline, hv);
    } else {
        return sub_32bit(vm, pipeline, hv);
    }
}

pub fn sub_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn sub_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn decrement_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError> {
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

pub fn decrement_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return decrement_16bit(vm, pipeline, hv);
    } else {
        return decrement_32bit(vm, pipeline, hv);
    }
}

pub fn decrement_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn decrement_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn cmp_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn cmp_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return cmp_16bit(vm, pipeline, hv);
    } else {
        return cmp_32bit(vm, pipeline, hv);
    }
}

pub fn cmp_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn cmp_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn test_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u8_exact()?;
    let result = base & mask;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign8(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    Ok(())
}

pub fn test_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return and_16bit(vm, pipeline, hv);
    } else {
        return and_32bit(vm, pipeline, hv);
    }
}

pub fn test_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u16_sx()?;
    let result = base & mask;
    vm.flags.calculate_zero(result as u32);
    vm.flags.calculate_parity(result as u32);
    vm.flags.calculate_sign16(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    Ok(())
}

pub fn test_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let mask = vm.get_arg(pipeline.args[1].location)?.u32_sx()?;
    let result = base & mask;
    vm.flags.calculate_zero(result);
    vm.flags.calculate_parity(result);
    vm.flags.calculate_sign32(result);
    vm.flags.carry = false;
    vm.flags.overflow = false;
    Ok(())
}


pub fn and_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn and_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return and_16bit(vm, pipeline, hv);
    } else {
        return and_32bit(vm, pipeline, hv);
    }
}

pub fn and_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn and_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn or_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn or_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return or_16bit(vm, pipeline, hv);
    } else {
        return or_32bit(vm, pipeline, hv);
    }
}

pub fn or_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn or_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn xor_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn xor_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return xor_16bit(vm, pipeline, hv);
    } else {
        return xor_32bit(vm, pipeline, hv);
    }
}

pub fn xor_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn xor_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
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

pub fn not_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    let result = !base;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn not_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return not_16bit(vm, pipeline, hv);
    } else {
        return not_32bit(vm, pipeline, hv);
    }
}

pub fn not_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    let result = !base;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn not_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    let result = !base;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn neg_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    vm.flags.carry = base != 0;
    let (result, overflow) = (base as i8).overflowing_neg();
    vm.flags.calculate_zero(result as u32);
    vm.flags.overflow = overflow;
    vm.set_arg(pipeline.args[0].location, SizedValue::Byte(result as u8))?;
    Ok(())
}

pub fn neg_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError> {
    if pipeline.size_override {
        return neg_16bit(vm, pipeline, hv);
    } else {
        return neg_32bit(vm, pipeline, hv);
    }
}

pub fn neg_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u16_exact()?;
    vm.flags.carry = base != 0;
    let (result, overflow) = (base as i16).overflowing_neg();
    vm.flags.calculate_zero(result as u32);
    vm.flags.overflow = overflow;
    vm.set_arg(pipeline.args[0].location, SizedValue::Word(result as u16))?;
    Ok(())
}

pub fn neg_32bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let base = vm.get_arg(pipeline.args[0].location)?.u32_exact()?;
    vm.flags.carry = base != 0;
    let (result, overflow) = (base as i32).overflowing_neg();
    vm.flags.calculate_zero(result as u32);
    vm.flags.overflow = overflow;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(result as u32))?;
    Ok(())
}

pub fn interrupt(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let num = vm.get_arg(pipeline.args[0].location)?.u8_exact()?;
    hv.interrupt(vm, num)?;
    Ok(())
}

pub fn setcc_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    if cc_matches(pipeline.opcode, &vm.flags){
        vm.set_arg(pipeline.args[0].location, SizedValue::Byte(1))?;
    }else{
        vm.set_arg(pipeline.args[0].location, SizedValue::Byte(0))?;
    }
    Ok(())
}

pub fn cmovcc_native(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    if cc_matches(pipeline.opcode, &vm.flags){
        vm.set_arg(pipeline.args[0].location, vm.get_arg(pipeline.args[1].location)?)?;
    }
    Ok(())
}

pub fn lea(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let address = vm.get_arg_lea(pipeline.args[1].location)?;
    let value = if pipeline.size_override{
        SizedValue::Word(address as u16)
    }else{
        SizedValue::Dword(address)
    };
    vm.set_arg(pipeline.args[0].location, value)?;
    Ok(())
}

pub fn movzx_8bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    if pipeline.size_override{
        let v = vm.get_arg(pipeline.args[1].location)?.u16_zx()?;
        vm.set_arg(pipeline.args[0].location, SizedValue::Word(v))?;
    }else{
        let v = vm.get_arg(pipeline.args[1].location)?.u32_zx()?;
        vm.set_arg(pipeline.args[0].location, SizedValue::Dword(v))?;
    }
    Ok(())
}

pub fn movzx_16bit(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let v = vm.get_arg(pipeline.args[1].location)?.u32_zx()?;
    vm.set_arg(pipeline.args[0].location, SizedValue::Dword(v))?;
    Ok(())
}

fn rep_no_flag_opcodes(opcode: u8) -> bool{
    match opcode{
        0xA4 | 0xA5 | //movs
        0xAC | 0xAD | //lods
        0xAA | 0xAB => { //stos
            true
        }
        _ => {
            false
        }
    }
}
fn rep_flag_opcodes(opcode: u8) -> bool{
    match opcode{
        0xA6 | 0xA7 | //cmps
        0xAE | 0xAF => { //scas
            true
        }
        _ => {
            false
        }
    }
}

fn read_regw(vm: &VM, reg: Reg32, size_override: bool) -> u32{
    if size_override{
        vm.regs[reg as usize] & 0x0000FFFF
    }else{
        vm.regs[reg as usize]
    }
}

fn decrement_regw(vm: &mut VM, reg: Reg32, size_override: bool) -> u32{
    if size_override{
        let mut r = (vm.regs[reg as usize] & 0x0000FFFF) as u16;
        r -= 1;
        let write = (vm.regs[reg as usize] & 0xFFFF0000) | (r as u32);
        vm.regs[reg as usize] = write;
        r as u32
    }else{
        vm.regs[reg as usize] -= 1;
        vm.regs[reg as usize]
    } 
}

pub fn repe(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let opcodes = &crate::opcodes::OPCODES;
    if rep_flag_opcodes(pipeline.opcode){
        unimplemented!();
    }else if rep_no_flag_opcodes(pipeline.opcode){
        let function = opcodes[pipeline.opcode as usize].opcodes[0].function;
        let gas_cost = vm.charger.cost(opcodes[pipeline.opcode as usize].opcodes[0].gas_cost);
        /*      
        while eCX <> 0
            execute string instruction once
            eCX . eCX - 1
        endwhile
        */
        while read_regw(vm, Reg32::ECX, pipeline.size_override) != 0{
            if vm.gas_remaining == 0{
                return Err(VMError::OutOfGas);
            }
            function(vm, pipeline, hv)?;
            decrement_regw(vm, Reg32::ECX, pipeline.size_override);
            vm.gas_remaining = vm.gas_remaining.saturating_sub(gas_cost);
        }
    }else{
        return Err(VMError::InvalidOpcodeEncoding);
    }
    Ok(())
}
pub fn repne(vm: &mut VM, pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    let opcodes = &crate::opcodes::OPCODES;
    if rep_flag_opcodes(pipeline.opcode){
        unimplemented!();
    }else{
        //note this prefix can not be used with non-flag using string instructions
        return Err(VMError::InvalidOpcodeEncoding);
    }
    Ok(())
}

pub fn movs_native_word(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    //[EDI] . [ESI]
    if pipeline.size_override{
        vm.set_mem(vm.reg32(Reg32::EDI), SizedValue::Word(vm.get_mem(vm.reg32(Reg32::ESI), ValueSize::Word)?.u16_exact()?))?;
        let d = if vm.flags.direction{
            (-2i32) as u32
        }else{
            2
        };
        //todo DF
        vm.set_reg32(Reg32::EDI, vm.reg32(Reg32::EDI).wrapping_add(d));
        vm.set_reg32(Reg32::ESI, vm.reg32(Reg32::ESI).wrapping_add(d));
    }else{
        vm.set_mem(vm.reg32(Reg32::EDI), SizedValue::Dword(vm.get_mem(vm.reg32(Reg32::ESI), ValueSize::Dword)?.u32_exact()?))?;
        //todo DF
        let d = if vm.flags.direction{
            (-4i32) as u32
        }else{
            4
        };
        vm.set_reg32(Reg32::EDI, vm.reg32(Reg32::EDI).wrapping_add(d));
        vm.set_reg32(Reg32::ESI, vm.reg32(Reg32::ESI).wrapping_add(d));
    }
    Ok(())
}
pub fn movsb(vm: &mut VM, pipeline: &Pipeline, hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    //[EDI] . [ESI]
    vm.set_mem(vm.reg32(Reg32::EDI), SizedValue::Byte(vm.get_mem(vm.reg32(Reg32::ESI), ValueSize::Byte)?.u8_exact()?))?;
    //todo DF
    let d = if vm.flags.direction{
        (-1i32) as u32
    }else{
        1
    };
    vm.set_reg32(Reg32::EDI, vm.reg32(Reg32::EDI).wrapping_add(d));
    vm.set_reg32(Reg32::ESI, vm.reg32(Reg32::ESI).wrapping_add(d));
    Ok(())
}
pub fn set_direction(vm: &mut VM, _pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    vm.flags.direction = true;
    Ok(())
}
pub fn clear_direction(vm: &mut VM, _pipeline: &Pipeline, _hv: &mut dyn Hypervisor) -> Result<(), VMError>{
    vm.flags.direction = false;
    Ok(())
}