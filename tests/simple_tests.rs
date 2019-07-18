extern crate qx86;
mod common;

use qx86::vm::*;
use common::*;
use qx86::structs::*;

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



