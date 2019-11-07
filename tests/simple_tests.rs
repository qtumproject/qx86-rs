extern crate qx86;
mod common;

use qx86::vm::*;
use common::*;
use qx86::structs::*;
use qx86::flags::*;
use std::default::*;

#[test]
fn test_undefined_opcode(){
    let mut vm = common::create_vm();
    let bytes = vec![
        0x90, //nop
        0x90,
        0x0F,
        0x0B,
        0x90,
        0x90
    ];
    vm.copy_into_memory(CODE_MEM, &bytes).unwrap();
    let mut hv = TestHypervisor::default();
    assert_eq!(vm.execute(&mut hv).err().unwrap(), VMError::InvalidOpcode(0x0F));
    assert_eq!(vm.error_eip, CODE_MEM + 2);
}

#[test]
fn test_simple_nop_hlt(){
    let mut vm = common::create_vm();
    let mut bytes = vec![];
    //use large block of nops to ensure it's larger than the pipeline size
    for _n in 0..100{
        bytes.push(0x90); //nop
    }
    bytes.push(0xF4); //hlt
    vm.copy_into_memory(CODE_MEM, &bytes).unwrap();
    let mut hv = TestHypervisor::default();
    assert!(vm.execute(&mut hv).unwrap());
    assert_eq!(vm.eip, CODE_MEM + 100);
}

#[test]
fn test_mov_hlt(){
    let vm = execute_vm_with_asm("
    mov al, 0x11
    mov ah, 0x22
    mov dl, 0x33
    mov bh, 0x44
    hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x00002211);
    assert_eq!(vm.reg8(Reg8::DL), 0x33);
    assert_eq!(vm.reg8(Reg8::BH), 0x44);
}
#[test]
fn test_mov(){
    //scratch memory: 0x80000000
    let vm = execute_vm_with_asm("
        mov al, 0x11
        mov ecx, 0x80000000
        mov dword [ecx], 0x11223344
        mov edi, 0x10
        mov dword [edi * 2 + ecx], 0x88776655
        mov byte [edi * 4 + ecx], 0xFF
        mov esp, [0x80000000]
        mov ah, [0x80000020]
        mov ebp, [edi * 2 + ecx]
        
        mov edx, 0x30
        mov dword [edx + 0x80000000], eax
        mov esi, 0x80000000
        mov ebx, dword [edx * 2 + esi]
        hlt"); 
    assert_eq!(vm.reg32(Reg32::ECX), DATA_MEM);
    assert_eq!(vm.reg8(Reg8::AL), 0x11);
    assert_eq!(vm.reg8(Reg8::AH), 0x55);
    assert_eq!(vm.reg32(Reg32::ESP), 0x11223344);
    assert_eq!(vm.reg32(Reg32::EBP), 0x88776655);
    assert_eq!(vm.get_mem(0x80000000, ValueSize::Dword).unwrap().u32_exact().unwrap(), 0x11223344);
    assert_eq!(vm.get_mem(0x10 * 2 + 0x80000000, ValueSize::Dword).unwrap().u32_exact().unwrap(), 0x88776655);
    assert_eq!(vm.get_mem(0x10 * 4 + 0x80000000, ValueSize::Byte).unwrap().u8_exact().unwrap(), 0xFF);
}

#[test]
fn test_push_pop(){
    let vm = execute_vm_with_asm("
        mov esp, 0x80000100
        push 0x12345678
        pop eax
        mov ebx, 0x80001000
        mov dword [ebx], 0xffeeddcc
        push dword [ebx]
        pop ecx
        push ebx
        hlt
    ");
    vm_diagnostics(&vm);
    assert_eq!(vm.reg32(Reg32::EAX), 0x12345678);
    assert_eq!(vm.reg32(Reg32::ECX), 0xffeeddcc);
    assert_eq!(vm.reg32(Reg32::ESP), 0x80000100 - 4);
    assert_eq!(vm.get_mem(0x80000100 - 4, ValueSize::Dword).unwrap().u32_exact().unwrap(), 0x80001000);
}

#[test]
fn test_jmp(){
    //This is hard to follow, but order is _a,_b,_c,_d,_e
    //This uses both long and short positive/negative jumps as well as an absolute jump
    let vm = execute_vm_with_asm("
    jmp short _a
    ud2 ;shouldn't reach here
    ud2
    _e:
    mov ebp, 3
    hlt ;EIP = org+7 + 5
    ud2 ;shouldn't reach here
    _c:
    mov esp, 4
    mov dword [eax], _e
    jmp long _d
    _b:
    mov esi, 5
    mov eax, 0x80000100
    jmp short _c

    _a:
    mov ecx, 1
    jmp long _b
    _d:
    mov edx, 2
    jmp [eax]
    ud2 ;shouldn't reach here
    ");
    vm_diagnostics(&vm);
    assert_eq!(vm.eip, CODE_MEM + 11);
    assert_eq!(vm.reg32(Reg32::EAX), 0x80000100);
    assert_eq!(vm.reg32(Reg32::ECX), 1);
    assert_eq!(vm.reg32(Reg32::EDX), 2);
    assert_eq!(vm.reg32(Reg32::EBP), 3);
    assert_eq!(vm.reg32(Reg32::ESP), 4);
    assert_eq!(vm.reg32(Reg32::ESI), 5);
}

#[test]
fn test_jcc(){
    let vm = execute_vm_with_asm("
    mov al, -120
    mov cl, 50
    cmp al, cl
    jo short _a
    ud2 ;shouldn't reach here
    ud2
    _e:
    mov ebp, 3
    hlt ;EIP = org+7 + 5
    ud2 ;shouldn't reach here
    _c:
    mov ecx, 0xFEFEFEFE
    jbe long _d
    _b:
    mov esi, 8
    mov eax, 8
    cmp eax, esi
    je short _c
    _a:
    mov eax, 0xF00090FF
    mov ebx, 0xF00121FA
    cmp eax, ebx
    jbe long _b
    _d:
    mov edx, 2
    jg short _e
    mov edx, 4
    jle short _e
    ud2 ;shouldn't reach here
    ");
    assert_eq!(vm.eip, CODE_MEM + 17);
    assert_eq!(vm.reg32(Reg32::EAX), 8);
    assert_eq!(vm.reg32(Reg32::ECX), 0xFEFEFEFE);
    assert_eq!(vm.reg32(Reg32::EDX), 4);
    assert_eq!(vm.reg32(Reg32::EBP), 3);
    assert_eq!(vm.reg32(Reg32::ESI), 8);
}

#[test]
fn test_override_jmp_error(){
    let mut vm = create_vm_with_asm("
    jmp word _a
    _a:
    hlt");
    let _ = execute_vm_with_error(&mut vm);
}

#[test]
fn test_jcxz() {
    let vm = execute_vm_with_asm("
        mov eax, 0
        jecxz short _a
        hlt
        _a:
        inc eax
        mov ecx, 1
        jecxz short _b
        hlt
        _b: 
        inc eax ; should not reach here
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 1);
}

#[test]
fn test_call_relw() {
    let vm = execute_vm_with_asm("
        mov esp, 0x80000100
        mov eax, 1
        call foobar
        noreach:
        ud2
        foobar:
        pop eax
        mov ebx, noreach
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), vm.reg32(Reg32::EBX));
}

#[test]
fn test_call_relw_with_reg() {
    let vm = execute_vm_with_asm("
        mov esp, 0x80000100
        mov eax, 1
        mov ecx, foobar
        call ecx
        noreach:
        ud2
        foobar:
        pop eax
        mov ebx, noreach
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), vm.reg32(Reg32::EBX));
}

#[test]
 fn test_ret() {
    let vm = execute_vm_with_asm("
        mov esp, 0x80000100
        mov eax, 1
        jmp skip
        ud2
        backward:
        mov eax, 2
        hlt
        skip:
        push backward
        ret");
    assert_eq!(vm.reg32(Reg32::EAX), 2);
    assert_eq!(vm.reg32(Reg32::ESP), 0x80000100);
}

#[test]
fn test_ret_with_optional_arg() {
    let vm = execute_vm_with_asm("
        mov esp, 0x80000100
        push dword 100
        call stack_sub
        hlt
        stack_sub:
        mov eax, [esp + 4]
        ret 4");
    assert_eq!(vm.reg32(Reg32::ESP), 0x80000100);
    assert_eq!(vm.reg32(Reg32::EAX), 100);
}

#[test]
fn test_divide_by_zero(){
    let mut vm = create_vm_with_asm("
    mov ax, 10
    mov bx, 0
    div bx
    hlt");
    assert_eq!(VMError::DivideByZero, execute_vm_with_error(&mut vm));
    vm = create_vm_with_asm("
    mov al, 5
    mov bl, 0
    div bl
    hlt");
    assert_eq!(VMError::DivideByZero, execute_vm_with_error(&mut vm));
    vm = create_vm_with_asm("
    mov edx, 1000
    mov eax, 1000
    mov ebx, 0
    div ebx
    hlt");
    assert_eq!(VMError::DivideByZero, execute_vm_with_error(&mut vm));
    vm = create_vm_with_asm("
    mov ax, 10
    mov bx, 0
    idiv bx
    hlt");
    assert_eq!(VMError::DivideByZero, execute_vm_with_error(&mut vm));
    vm = create_vm_with_asm("
    mov al, 5
    mov bl, 0
    idiv bl
    hlt");
    assert_eq!(VMError::DivideByZero, execute_vm_with_error(&mut vm));
    vm = create_vm_with_asm("
    mov edx, 1000
    mov eax, 1000
    mov ebx, 0
    idiv ebx
    hlt");
    assert_eq!(VMError::DivideByZero, execute_vm_with_error(&mut vm));
}

#[test]
fn test_quotient_and_remainder_8bit(){
    let vm = execute_vm_with_asm("
        mov al, 100
        mov bl, 15
        div bl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 6);
    assert_eq!(vm.reg8(Reg8::AH), 10);
}

#[test]
fn test_quotient_and_remainder_16bit(){
    let vm = execute_vm_with_asm("
        mov ax, 1000
        mov bx, 200
        div bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 5);
    assert_eq!(vm.reg16(Reg16::DX), 0);
}

#[test]
fn test_quotient_and_remainder_32bit(){
    let vm = execute_vm_with_asm("
        mov eax, 1000
        mov ebx, 200
        div ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 5);
    assert_eq!(vm.reg32(Reg32::EDX), 0);
}

#[test]
fn test_quotient_and_remainder_8bit_idiv(){
    let vm = execute_vm_with_asm("
        mov al, 100
        mov bl, 15
        idiv bl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 6);
    assert_eq!(vm.reg8(Reg8::AH), 10);
}

#[test]
fn test_xchg_8bit() {
    let vm = execute_vm_with_asm("
        mov al, 100
        mov bl, 200
        xchg al, bl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 200);
    assert_eq!(vm.reg8(Reg8::BL), 100);
}

#[test]
fn test_xchg_16bit() {
    let vm = execute_vm_with_asm("
        mov cx, 0xFFFF
        mov bx, 0xFFEE
        xchg cx, bx
        hlt");
    assert_eq!(vm.reg16(Reg16::CX), 0xFFEE);
    assert_eq!(vm.reg16(Reg16::BX), 0xFFFF);
}

#[test]
fn test_xchg_32bit() {
    let vm = execute_vm_with_asm("
        mov eax, 0xFFFFFFFF
        mov ebx, 0xFFFFEEEE
        xchg eax, ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xFFFFEEEE);
    assert_eq!(vm.reg32(Reg32::EBX), 0xFFFFFFFF);
}

#[test]
fn test_quotient_and_remainder_16bit_idiv(){
    let vm = execute_vm_with_asm("
        mov ax, 1000
        mov bx, 200
        idiv bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 5);
    assert_eq!(vm.reg16(Reg16::DX), 0);
}

#[test]
fn test_quotient_and_remainder_32bit_idiv(){
    let vm = execute_vm_with_asm("
        mov eax, 1000
        mov ebx, 200
        idiv ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 5);
    assert_eq!(vm.reg32(Reg32::EDX), 0);
}

#[test]
fn test_shl_sign_carry_flag(){
    let vm = execute_vm_with_asm("
        mov eax, 0x007f8000
        shl eax, 10
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xFE000000);
    assert_eq!(vm.flags, X86Flags{carry:true, sign:true, parity:true, ..Default::default()});
}

#[test]
fn test_shl_zero_overflow_carry_flag(){
    let vm = execute_vm_with_asm("
        mov eax, 0x80000000
        shl eax, 1
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0);
    assert_eq!(vm.flags, X86Flags{carry:true, zero: true, overflow: true, parity:true, ..Default::default()});
}

#[test]
fn test_shl_8bit(){
    let vm = execute_vm_with_asm("
        mov al, 8
        shl al, 2
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x20);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_shl_8bit_sign(){
    let vm = execute_vm_with_asm("
        mov al, 0x20
        shl al, 2
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x80);
    assert_eq!(vm.flags, X86Flags{sign: true, ..Default::default()});
}

#[test]
fn test_shl_8bit_overflow(){
    let vm = execute_vm_with_asm("
        mov al, 0x80
        shl al, 1
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0);
    assert_eq!(vm.flags, X86Flags{overflow: true, carry: true, parity: true, zero: true, ..Default::default()});
}

#[test]
fn test_shr_8bit_carry(){
    let vm = execute_vm_with_asm("
        mov al, 0xb
        shr al, 1
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 5);
    assert_eq!(vm.flags, X86Flags{carry: true, parity: true, ..Default::default()});
}

#[test]
fn test_shr_8bit_carry_overflow(){
    let vm = execute_vm_with_asm("
        mov al, 0xFF
        shr al, 1
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x7F);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, ..Default::default()});
}

#[test]
fn test_shr_16bit_carry(){
    let vm = execute_vm_with_asm("
        mov ax, 0x5DDD
        shr ax, 1
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x2EEE);
    assert_eq!(vm.flags, X86Flags{carry: true, parity: true, ..Default::default()});
}

#[test]
fn test_shr_16bit_carry_overflow(){
    let vm = execute_vm_with_asm("
        mov ax, 0xFFFF
        shr ax, 1
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x7FFF);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, parity: true, ..Default::default()});
}

#[test]
fn test_regular_mul8bit_mul_0() {
    let vm = execute_vm_with_asm("
        mov al, 1
        mul bl
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_regular_mul8bit_basic(){
    let vm = execute_vm_with_asm("
        mov al, 0xFF
        mov bl, 0xFF
        mul bl
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0xFE01);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, ..Default::default()});
}

#[test]
fn test_regular_mul8bit_basic_no_flag(){
    let vm = execute_vm_with_asm("
        mov al, 0x01
        mov bl, 0x04
        mul bl
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 4);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_regular_mul16bit_basic() {
    let vm = execute_vm_with_asm("
        mov ax, 0xFFFF
        mov bx, 0xFFFF
        mul bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 1);
    assert_eq!(vm.reg16(Reg16::DX), 0xFFFE);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, ..Default::default()});
}

#[test]
fn test_regular_mul16bit_basic_no_flag() {
    let vm = execute_vm_with_asm("
        mov ax, 2
        mov bx, 0xFE
        mul bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x1FC);
    assert_eq!(vm.reg16(Reg16::DX), 0);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_regular_mul32bit_basic() {
    let vm = execute_vm_with_asm("
        mov eax, 0xFFFFFFFF
        mov ebx, 0xFFFFFFFF
        mul ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 1);
    assert_eq!(vm.reg32(Reg32::EDX), 0xFFFFFFFE);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, ..Default::default()});
}

#[test]
fn test_regular_mul32bit_basic_no_flag() {
    let vm = execute_vm_with_asm("
        mov eax, 2
        mov ebx, 0xFFFFEE
        mul ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x1FFFFDC);
    assert_eq!(vm.reg32(Reg32::EDX), 0);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_regular_imul8bit_imul_0() {
    let vm = execute_vm_with_asm("
        mov al, 1
        imul bl
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_regular_imul8bit_basic(){
    let vm = execute_vm_with_asm("
        mov al, 100
        mov bl, -100
        imul bl
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0xD8F0);
    assert_eq!(vm.flags, X86Flags{overflow: true, carry: true, ..Default::default()});
}

#[test]
fn test_regular_imul8bit_basic_no_flag(){
    let vm = execute_vm_with_asm("
        mov al, 0x01
        mov bl, 0x04
        imul bl
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 4);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_regular_imul16bit_basic() {
    let vm = execute_vm_with_asm("
        mov ax, 10000
        mov bx, -10000
        imul bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x1F00);
    assert_eq!(vm.reg16(Reg16::DX), 0xFA0A);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, ..Default::default()});
}

#[test]
fn test_regular_imul16bit_basic_no_flag() {
    let vm = execute_vm_with_asm("
        mov ax, 0xFFFF
        mov bx, 0xFFFF
        imul bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x1);
    assert_eq!(vm.reg16(Reg16::DX), 0);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_regular_imul32bit_basic() {
    let vm = execute_vm_with_asm("
        mov eax, 1000000000
        mov ebx, -1000000000
        imul ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x589C0000);
    assert_eq!(vm.reg32(Reg32::EDX), 0xF21F494C);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, ..Default::default()});
}

#[test]
fn test_regular_imul32bit_basic_no_flag() {
    let vm = execute_vm_with_asm("
        mov eax, 2
        mov ebx, 0xFFFFEE
        imul ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x1FFFFDC);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_regular_imul32bit_two_args_flag() {
    let vm = execute_vm_with_asm("
        mov eax, 3456
        mov ecx, 4290403
        imul eax, ecx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x73CBB880);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, ..Default::default()});
}

#[test]
fn test_regular_imul32bit_three_args_flag() {
    let vm = execute_vm_with_asm("
        mov eax, 3456
        imul ebx, eax, 4290403
        hlt");
    assert_eq!(vm.reg32(Reg32::EBX), 0x73CBB880);
    assert_eq!(vm.flags, X86Flags{carry: true, overflow: true, ..Default::default()});
}

#[test]
fn test_signed_carry_add32(){
    let vm = execute_vm_with_asm("
        mov eax, 0xF00090FF
        mov ebx, 0xF00121FA
        add eax, ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xE001B2F9);
    assert_eq!(vm.flags, X86Flags{carry: true, parity: true, adjust: true, sign: true, ..Default::default()});
}

#[test]
fn test_signed_carry_adc32(){
    let vm = execute_vm_with_asm("
        mov eax, 0xF00090FF
        mov ebx, 0xF00121FA
        add eax, ebx
        adc eax, ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xD002D4F4);
    assert_eq!(vm.flags, X86Flags{carry: true, adjust: true, sign: true, ..Default::default()});
}

#[test]
fn test_overflow_signed_add32(){
    let vm = execute_vm_with_asm("
        mov eax, 0x7FFFFFFF
        mov ebx, 0x7FFFFFFF
        add eax, ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xFFFFFFFE);
    assert_eq!(vm.flags, X86Flags{overflow: true, adjust: true, sign: true, ..Default::default()});
}

#[test]
fn test_overflow_signed_adc32(){
    let vm = execute_vm_with_asm("
        mov eax, 0x7FFFFFFF
        mov ebx, 0x7FFFFFFF
        add eax, ebx
        adc eax, ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x7FFFFFFD);
    assert_eq!(vm.flags, X86Flags{carry: true, adjust: true, ..Default::default()});
}

#[test]
fn test_simple_add16(){
    let vm = execute_vm_with_asm("
        mov ax, 0x0064
        mov bx, 0x0320
        add ax, bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x0384);
    assert_eq!(vm.flags, X86Flags{parity: true, ..Default::default()});
}

#[test]
fn test_signed_zero_add8(){
    let vm = execute_vm_with_asm("
        mov al, 155
        mov cl, 101
        add al, cl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0);
    assert_eq!(vm.flags, X86Flags{carry: true, zero: true, adjust: true, parity: true, ..Default::default()});
}

#[test]
fn test_signed_zero_adc8(){
    let vm = execute_vm_with_asm("
        mov al, 155
        mov cl, 101
        add al, cl
        adc al, cl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x66);
    assert_eq!(vm.flags, X86Flags{parity: true, ..Default::default()});
}

#[test]
fn test_32bit_8bit_add(){
    let vm = execute_vm_with_asm("
        add eax, byte -1
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xFFFFFFFF);
    assert_eq!(vm.flags, X86Flags{sign: true, parity: true, ..Default::default()});
}

#[test]
fn test_16bit_8bit_add() {
    let vm = execute_vm_with_asm("
        add ax, byte -1
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0xFFFF);
    assert_eq!(vm.flags, X86Flags{sign: true, parity: true, ..Default::default()});
}

#[test]
fn test_unsigned_8bit_sub(){
    let vm = execute_vm_with_asm("
        mov al, 155
        mov cl, 101
        sub al, cl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x36);
    assert_eq!(vm.flags, X86Flags{overflow: true, parity: true, ..Default::default()});
}

#[test]
fn test_negative_unsigned_16bit_sub(){
    let vm = execute_vm_with_asm("
        mov ax, 100
        mov bx, 800
        sub ax, bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0xFD44);
    assert_eq!(vm.flags, X86Flags{carry: true, sign: true, parity: true, ..Default::default()});
}

#[test]
fn test_subtracting_negatives_32bit_sub(){
    let vm = execute_vm_with_asm("
        mov eax, 0xF00090FF
        mov ebx, 0xF00121FA
        sub eax, ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xFFFF6F05);
    assert_eq!(vm.flags, X86Flags{carry: true, sign: true, parity: true, ..Default::default()});
}

#[test]
fn test_achieving_zero_with_subtraction_32bit_sub(){
    let vm = execute_vm_with_asm("
        mov eax, 0x7FFFFFFF
        mov ebx, 0x7FFFFFFF
        sub eax, ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x0);
    assert_eq!(vm.flags, X86Flags{zero: true, parity: true, ..Default::default()});
}

#[test]
fn test_subtracting_negatives_8bit_sub(){
    let vm = execute_vm_with_asm("
        mov al, 0xFA
        mov cl, 0xFF
        sub al, cl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0xFB);
    assert_eq!(vm.flags, X86Flags{carry: true, sign: true, adjust: true, ..Default::default()});
}

#[test]
fn test_signed_subtraction_8bit_sub(){
    let vm = execute_vm_with_asm("
        mov al, 0xFE
        mov cl, 0xFF
        sub al, cl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0xFF);
    assert_eq!(vm.flags, X86Flags{carry: true, sign: true, adjust: true, parity: true, ..Default::default()});
}

#[test]
fn test_negative_addition_8bit_sub(){
    let vm = execute_vm_with_asm("
        mov al, -120
        mov cl, 50
        sub al, cl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x56);
    assert_eq!(vm.flags, X86Flags{overflow: true, parity: true, ..Default::default()});
}

#[test]
fn test_signed_comparison_8bit_cmp(){
    let vm = execute_vm_with_asm("
        mov al, 0xFE
        mov cl, 0xFF
        cmp al, cl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0xFE);
    assert_eq!(vm.flags, X86Flags{carry: true, sign: true, adjust: true, parity: true, ..Default::default()});
}

#[test]
fn test_inc_and_dec_8bit_and_32bit() {
    let vm = execute_vm_with_asm("
        mov al, 0xFE
        inc al
        mov ebx, 0xDEADBEEF
        inc ebx
        mov cl, 0xFE
        dec cl
        mov edx, 0xDEADBEEF
        dec edx
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0xFF);
    assert_eq!(vm.reg32(Reg32::EBX), 0xDEADBEF0);
    assert_eq!(vm.reg8(Reg8::CL), 0xFD);
    assert_eq!(vm.reg32(Reg32::EDX), 0xDEADBEEE);
    assert_eq!(vm.flags, X86Flags{parity:true, sign: true, ..Default::default()});
}

#[test]
fn test_inc_dont_modify_carry_flag() {
    let vm = execute_vm_with_asm("
        mov eax, 0xFFFFFFFF
        inc eax
        hlt");
        assert_eq!(vm.flags, X86Flags{zero: true, parity: true, adjust: true, ..Default::default()});
}

#[test]
fn test_dec_dont_modify_carry_flag() {
    let vm = execute_vm_with_asm("
        dec eax
        hlt");
    assert_eq!(vm.flags, X86Flags{sign: true, parity: true, adjust: true, ..Default::default()});
}

#[test]
fn test_and_rm8_r8(){
    let vm = execute_vm_with_asm("
        mov AL, 0xFF
        mov BL, 0xA7
        and AL, BL
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0xA7);
    assert_eq!(vm.flags, X86Flags{sign: true, ..Default::default()});
}

#[test]
fn test_and_rmw_rw() {
    let vm = execute_vm_with_asm("
        mov AX, 0xFFFF
        mov BX, 0xC8A7
        and AX, BX
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0xC8A7);
    assert_eq!(vm.flags, X86Flags{sign: true, ..Default::default()});
}

#[test]
fn test_and_r8_rm8() {
    let vm = execute_vm_with_asm("
        mov AL, 0xFF
        mov EBX, _tmp
        and AL, [EBX]
        hlt
        _tmp: dB 0xA7, 0, 0, 0
    ");
    assert_eq!(vm.reg8(Reg8::AL), 0xA7);
    assert_eq!(vm.flags, X86Flags{sign: true, ..Default::default()});
}

#[test]
fn test_and_ax_immw() {
    let vm = execute_vm_with_asm("
        mov AX, 0xFFFF
        and AX, 0xA7A7
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0xA7A7);
    assert_eq!(vm.flags, X86Flags{sign: true, ..Default::default()});
}

#[test]
fn test_or_parity_sign_8bit(){
     let vm = execute_vm_with_asm("
        mov AL, 0x16
        mov BL, 0x89
        or AL, BL
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x9F);
    assert_eq!(vm.flags, X86Flags{sign: true, parity: true, ..Default::default()});   
}

#[test]
fn test_or_8bit(){
     let vm = execute_vm_with_asm("
        mov AL, 0x76
        mov BL, 0x09
        or AL, BL
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x7F);
    assert_eq!(vm.flags, X86Flags{..Default::default()});   
}

#[test]
fn test_or_parity_zero_8bit(){
     let vm = execute_vm_with_asm("
        mov AL, 0x0
        mov BL, 0x0
        or AL, BL
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0x0);
    assert_eq!(vm.flags, X86Flags{zero: true, parity: true, ..Default::default()});   
}

#[test]
fn test_or_parity_sign_16bit(){
     let vm = execute_vm_with_asm("
        mov AX, 0x1616
        mov BX, 0x8989
        or AX, BX
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x9F9F);
    assert_eq!(vm.flags, X86Flags{sign: true, parity: true, ..Default::default()});   
}

#[test]
fn test_or_16bit(){
     let vm = execute_vm_with_asm("
        mov AX, 0x7676
        mov BX, 0x0909
        or AX, BX
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x7F7F);
    assert_eq!(vm.flags, X86Flags{..Default::default()});   
}

#[test]
fn test_or_parity_zero_16bit(){
     let vm = execute_vm_with_asm("
        mov AX, 0x0
        mov BX, 0x0
        or AX, BX
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x0);
    assert_eq!(vm.flags, X86Flags{zero: true, parity: true, ..Default::default()});   
}

#[test]
fn test_or_parity_sign_32bit(){
     let vm = execute_vm_with_asm("
        mov EAX, 0x16161616
        mov EBX, 0x89898989
        or EAX, EBX
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x9F9F9F9F);
    assert_eq!(vm.flags, X86Flags{sign: true, parity: true, ..Default::default()});   
}

#[test]
fn test_or_32bit(){
     let vm = execute_vm_with_asm("
        mov EAX, 0x76767676
        mov EBX, 0x09090909
        or EAX, EBX
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x7F7F7F7F);
    assert_eq!(vm.flags, X86Flags{..Default::default()});   
}

#[test]
fn test_or_parity_zero_32bit(){
     let vm = execute_vm_with_asm("
        mov EAX, 0x0
        mov EBX, 0x0
        or EAX, EBX
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x0);
    assert_eq!(vm.flags, X86Flags{zero: true, parity: true, ..Default::default()});
}

#[test]
fn test_xor() {
    let vm = execute_vm_with_asm("
        mov DL, 0xFF
        xor DL, 0x01
        hlt");
    assert_eq!(vm.reg8(Reg8::DL), 0xFE);
    assert_eq!(vm.flags, X86Flags{sign: true, ..Default::default()});
}

#[test]
fn test_not() {
    let vm = execute_vm_with_asm("
        mov AL, 0xFA
        not AL
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 5);
    assert_eq!(vm.flags, X86Flags::default());
}

#[test]
fn test_neg() {
    let vm = execute_vm_with_asm("
        mov AL, 0xFA
        neg AL
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 6);
    assert_eq!(vm.flags, X86Flags{carry: true, ..Default::default()});
}

#[test]
fn test_neg_zero() {
    let vm = execute_vm_with_asm("
        neg AL
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0);
     assert_eq!(vm.flags, X86Flags{zero: true, ..Default::default()});
}

#[test]
fn test_interrupt(){
    let mut hv = TestHypervisor::default();
    let vm = execute_vm_with_asm_and_hypervisor("
        mov ebx, 0x11223344
        int 0xAA
        mov ebx, 0xFFEEDDCC
        int 0xAA
        int 0xBB
        int3
        hlt
    ", &mut hv);
    assert_eq!(hv.pushed_values[0], 0x11223344);
    assert_eq!(hv.pushed_values[1], 0xFFEEDDCC);
    assert_eq!(hv.ints_triggered[0], 0xAA);
    assert_eq!(hv.ints_triggered[1], 0xAA);
    assert_eq!(hv.ints_triggered[2], 0xBB);
    assert_eq!(hv.ints_triggered[3], 3);
}

#[test]
fn test_setcc() {
    let vm = execute_vm_with_asm("
        mov AL, 20
        mov BL, 10
        mov CL, 30
        cmp AL, 20
        sete BL
        setne CL
        hlt");
    assert_eq!(vm.reg8(Reg8::BL), 1);
    assert_eq!(vm.reg8(Reg8::CL), 0);
}
#[test]
fn test_movcc() {
    let vm = execute_vm_with_asm("
        mov AL, 20
        cmp AL, 20
        mov EBX, 0x11223344
        mov ECX, 0x55667788
        mov EDX, 0x99AABBCC
        mov ESI, 0xDDEEFF11
        cmove EBX, ECX
        cmovne EDX, ESI
        hlt");
    assert_eq!(vm.reg32(Reg32::EBX), 0x55667788);
    assert_eq!(vm.reg32(Reg32::ECX), 0x55667788);
    assert_eq!(vm.reg32(Reg32::EDX), 0x99AABBCC);
    assert_eq!(vm.reg32(Reg32::ESI), 0xDDEEFF11);
}

#[test]
fn test_lea() {
    let vm = execute_vm_with_asm("
        mov eax, 0
        mov ebx, 5
        lea eax, [ebx * 2 + 1000]
        lea dx, [ebx * 2 + 100000]
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 5 * 2 + 1000);
    assert_eq!(vm.reg32(Reg32::EDX), (5 * 2 + 100000) & 0x0000FFFF);
}

#[test]
fn test_movzx() {
    let vm = execute_vm_with_asm("
        mov eax, 0xFA
        mov ebx, 0xFFFFFFFF
        mov ecx, 0xFFFFFFFF
        mov edx, 0xFFFFFFFF
        mov esi, 0xffffffff
        movzx ebx, al
        movzx cx, al
        mov dx, 0xFEDC
        movzx esi, dx 
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xFA);
    assert_eq!(vm.reg32(Reg32::EBX), 0xFA);
    assert_eq!(vm.reg32(Reg32::ECX), 0xFFFF00FA);
    assert_eq!(vm.reg32(Reg32::EDX), 0xFFFFFEDC);
    assert_eq!(vm.reg32(Reg32::ESI), 0xFEDC);
}

#[test]
fn test_rep_movsb() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov esi, 0x80000002
        mov dword [esi], 0x11223344
        mov dword [edi], 0xaabbccdd
        mov ecx, 4
        rep movsb
        mov eax, [0x80000000]
        mov ebx, [0x80000002]
        hlt");      
    /*
        ; memory before
        ; 00 00 44 33 22 11 -> dd cc bb aa 22 11
        ; aa bb cc dd 22 11
        ; memory after at 8..0
        ; bb aa 22 11
    */
    assert_eq!(vm.reg32(Reg32::EAX), 0x1122aabb);
    assert_eq!(vm.reg32(Reg32::EBX), 0x11221122);
    assert_eq!(vm.reg32(Reg32::ESI), 0x80000006);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000004);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
}

#[test]
fn test_rep_movsb_df() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov dword [edi], 0x11223344
        mov dword [edi + 4], 0xaabbccdd
        mov esi, 0x80000006
        mov edi, 0x80000004 
        mov ecx, 4
        std
        rep movsb
        mov eax, [0x80000000]
        mov ebx, [0x80000004]
        hlt");      
    /*
        ; memory before
        ; 44 33 22 11 dd cc bb aa
        ; memory after
        ; 44 cc bb cc bb cc bb aa
    */
    assert_eq!(vm.reg32(Reg32::EAX), 0xccbbcc44);
    assert_eq!(vm.reg32(Reg32::EBX), 0xaabbccbb);
    assert_eq!(vm.reg32(Reg32::ESI), 0x80000002);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000000);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
}

#[test]
fn test_rep_movsw() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov dword [edi], 0x11223344
        mov dword [edi + 4], 0xaabbccdd
        mov esi, 0x80000002
        mov ecx, 4
        rep movsw
        mov eax, [0x80000000]
        mov ebx, [0x80000004]
        hlt");      
    /*
        ; memory before
        ; 44 33 22 11 dd cc bb aa
        ; memory after at 0..8
        ; 22 11 dd cc bb aa 00 00
    */
    assert_eq!(vm.reg32(Reg32::EAX), 0xccdd1122);
    assert_eq!(vm.reg32(Reg32::EBX), 0x0000aabb);
    assert_eq!(vm.reg32(Reg32::ESI), 0x8000000a);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000008);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
}

#[test]
fn test_rep_movsd() {
    let vm = execute_vm_with_asm("
        mov esi, 0x80000000
        mov dword [esi], 0x11223344
        mov dword [esi + 4], 0xaabbccdd
        mov dword [esi + 8], 0x55667788
        mov edi, 0x80000004
        mov ecx, 3
        rep movsd
        mov eax, [0x80000004]
        mov ebx, [0x80000008]
        mov edx, [0x8000000C]
        hlt");      
    assert_eq!(vm.reg32(Reg32::EAX), 0x11223344);
    assert_eq!(vm.reg32(Reg32::EBX), 0x11223344);
    assert_eq!(vm.reg32(Reg32::EDX), 0x11223344);
    assert_eq!(vm.reg32(Reg32::ESI), 0x8000000C);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000010);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
}

#[test]
fn test_repe_cmpsb() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov esi, 0x80000002
        mov dword [esi], 0xaaaaaaaa
        mov dword [edi], 0xaaaaaadd
        mov ecx, 4
        repe cmpsb
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x80000003);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000001);
    assert_eq!(vm.reg32(Reg32::ECX), 3);
    assert_eq!(vm.flags, X86Flags{carry: true, sign: true, adjust: true, ..Default::default()});
}

#[test]
fn test_repne_cmpsb() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov esi, 0x80000002
        mov dword [esi], 0x11223344
        mov dword [edi], 0xaabbccdd
        mov ecx, 4
        repne cmpsb
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x80000006);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000004);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
    assert_eq!(vm.flags, X86Flags{carry: true, adjust: true, ..Default::default()});
}

#[test]
fn test_repne_cmpsw() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov dword [edi], 0x11223344
        mov dword [edi + 4], 0xaabbccdd
        mov esi, 0x80000004
        mov ecx, 4
        repne cmpsw
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x8000000c);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000008);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
    // carry is being triggered here
    assert_eq!(vm.flags, X86Flags{carry: true, adjust: true, ..Default::default()});
}

#[test]
fn test_repne_cmpsw_2() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov esi, 0x80000004
        mov dword [edi], 0x11223344
        mov dword [esi], 0xaabbccdd
        mov ecx, 3
        repne cmpsw
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x8000000a);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000006);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
    // carry is being triggered here
    assert_eq!(vm.flags, X86Flags{carry: true, adjust: true, ..Default::default()});
}

#[test]
fn test_repe_cmpsw() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov dword [edi], 0x11223344
        mov dword [edi + 4], 0x11223344
        mov esi, 0x80000004
        mov ecx, 4
        repe cmpsw
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x8000000a);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000006);
    assert_eq!(vm.reg32(Reg32::ECX), 1);
    assert_eq!(vm.flags, X86Flags{carry: true, sign: true, adjust: true, ..Default::default()});
}

#[test]
fn test_repne_cmpsd() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov dword [edi], 0xaabbccdd
        mov dword [edi + 4], 0xaabbccdd
        mov dword [edi + 8], 0xeeffaabb
        mov esi, 0x80000004
        mov ecx, 3
        repne cmpsd
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x80000008);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000004);
    assert_eq!(vm.reg32(Reg32::ECX), 2);
    assert_eq!(vm.flags, X86Flags{ zero: true, parity: true, ..Default::default()});
}

#[test]
fn test_repe_cmpsd() {
    let vm = execute_vm_with_asm("
        mov edi, 0x80000000
        mov dword [edi], 0xaabbccdd
        mov dword [edi + 4], 0xaabbccdd
        mov dword [edi + 8], 0xeeffaabb
        mov esi, 0x80000004
        mov ecx, 3
        repe cmpsd
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x8000000c);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000008);
    assert_eq!(vm.reg32(Reg32::ECX), 1);
    assert_eq!(vm.flags, X86Flags{ adjust: true, parity: true, ..Default::default()});
}

#[test]
fn test_lodsb_stosb() {
    let vm = execute_vm_with_asm("
        mov esi, 0x80000000
        mov edi, 0x80000004
        mov byte [esi], 0x08
        mov ecx, 1
        rep lodsb
        mov ecx, 3
        rep stosb
        mov ebx, dword [edi - 3]
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x80000001);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000007);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
    assert_eq!(vm.reg32(Reg32::EAX), 0x00000008);
    assert_eq!(vm.reg32(Reg32::EBX), 0x00080808);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_lodsw_stosw() {
    let vm = execute_vm_with_asm("
        mov esi, 0x80000000
        mov edi, 0x80000004
        mov word [esi], 0x8008
        mov ecx, 1
        rep lodsw
        mov ecx, 2
        rep stosw
        mov ebx, dword [edi - 4]
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x80000002);
    assert_eq!(vm.reg32(Reg32::EDI), 0x80000008);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
    assert_eq!(vm.reg32(Reg32::EAX), 0x00008008);
    assert_eq!(vm.reg32(Reg32::EBX), 0x80088008);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_lodsd_stosd() {
    let vm = execute_vm_with_asm("
        mov esi, 0x80000000
        mov edi, 0x80000008
        mov dword [esi], 0x8080BEAD
        mov ecx, 1
        rep lodsd
        mov ecx, 1
        rep stosd
        mov ebx, dword [edi - 4]
        hlt");
    assert_eq!(vm.reg32(Reg32::ESI), 0x80000004);
    assert_eq!(vm.reg32(Reg32::EDI), 0x8000000c);
    assert_eq!(vm.reg32(Reg32::ECX), 0);
    assert_eq!(vm.reg32(Reg32::EAX), 0x8080bead);
    assert_eq!(vm.reg32(Reg32::EBX), 0x8080bead);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}

#[test]
fn test_enter_leave() {
    let vm = execute_vm_with_asm("
        mov ebp, 0x8000000A
        mov esp, 0x80006650
        enter 1, 0
        mov eax, [esp + 1]
        mov ebx, esp
        mov ecx, ebp
        leave
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0x8000000A);
    assert_eq!(vm.reg32(Reg32::EBX), 0x8000664b);
    assert_eq!(vm.reg32(Reg32::ECX), 0x8000664C);
    /**
     * Todo: add these statements after enter 1, 0 and figure out why
     * the while loop portion is erroring
     *  enter 1, 2
     *  mov edx, [esp + 1]
     *  mov esi, esp
     *  mov edi, ebp
     * also add in these assertions
     * assert_eq!(vm.reg32(Reg32::EDI), 0x8000664b);
     * assert_eq!(vm.reg32(Reg32::ESI), 0x8000664C);    
     */
    assert_eq!(vm.reg32(Reg32::ESP), 0x80006650);
    assert_eq!(vm.reg32(Reg32::EBP), 0x8000000A);
    assert_eq!(vm.flags, X86Flags{..Default::default()});
}