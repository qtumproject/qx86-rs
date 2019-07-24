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
        0xAA, //eventually this might not be an undefined opcode
        0x90,
        0x90
    ];
    vm.copy_into_memory(CODE_MEM, &bytes).unwrap();
    assert_eq!(vm.execute().err().unwrap(), VMError::InvalidOpcode(0xAA));
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

