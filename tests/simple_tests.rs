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
        0xFF, //eventually this might not be an undefined opcode
        0x90,
        0x90
    ];
    vm.copy_into_memory(CODE_MEM, &bytes).unwrap();
    assert!(vm.execute().err().unwrap() == VMError::InvalidOpcode);
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
    assert!(vm.execute().unwrap());
    assert_eq!(vm.eip, CODE_MEM + 100);
}

#[test]
fn test_mov_hlt(){
    let vm = execute_vm_asm("
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
    let vm = execute_vm_asm("
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
fn test_signed_carry_add32(){
    let vm = execute_vm_asm("
        mov eax, 0xF00090FF
        mov ebx, 0xF00121FA
        add eax, ebx
        hlt");
    //assert_eq!(vm.reg8(Reg8::AL), 0x77);
    assert_eq!(vm.reg32(Reg32::EAX), 0xE001B2F9);
    assert_eq!(vm.flags, X86Flags{carry: true, parity: true, adjust: true, sign: true, ..Default::default()});
}

#[test]
fn test_overflow_signed_add32(){
    let vm = execute_vm_asm("
        mov eax, 0x7FFFFFFF
        mov ebx, 0x7FFFFFFF
        add eax, ebx
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xFFFFFFFE);
    assert_eq!(vm.flags, X86Flags{overflow: true, adjust: true, sign: true, ..Default::default()});
}

#[test]
fn test_simple_add16(){
    let vm = execute_vm_asm("
        mov ax, 0x0064
        mov bx, 0x0320
        add ax, bx
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0x0384);
    assert_eq!(vm.flags, X86Flags{parity: true, ..Default::default()});
}

#[test]
fn test_signed_zero_add8(){
    let vm = execute_vm_asm("
        mov al, 155
        mov cl, 101
        add al, cl
        hlt");
    assert_eq!(vm.reg8(Reg8::AL), 0);
    assert_eq!(vm.flags, X86Flags{carry: true, zero: true, adjust: true, parity: true, ..Default::default()});
}

#[test]
fn test_32bit_8bit_add(){
    let vm = execute_vm_asm("
        add eax, byte -1
        hlt");
    assert_eq!(vm.reg32(Reg32::EAX), 0xFFFFFFFF);
    assert_eq!(vm.flags, X86Flags{sign: true, parity: true, ..Default::default()});
}

#[test]
fn test_16bit_8bit_add() {
    let vm = execute_vm_asm("
        add ax, byte -1
        hlt");
    assert_eq!(vm.reg16(Reg16::AX), 0xFFFF);
    assert_eq!(vm.flags, X86Flags{sign: true, parity: true, ..Default::default()});
}