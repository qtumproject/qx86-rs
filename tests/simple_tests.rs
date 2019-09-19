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
        0xAA, //eventually this might not be an undefined opcode
        0x90,
        0x90
    ];
    vm.copy_into_memory(CODE_MEM, &bytes).unwrap();
    let mut hv = TestHypervisor::default();
    assert_eq!(vm.execute(&mut hv).err().unwrap(), VMError::InvalidOpcode(0xAA));
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

