extern crate qx86;
mod common;

use qx86::vm::*;
use common::*;


#[test]
pub fn fibonacci(){
    let mut hv = TestHypervisor::default();
    hv.pushed_values.push(10);
    hv.pushed_values.push(1);
    hv.pushed_values.push(0);
    hv.pushed_values.push(2);
    //based on https://rosettacode.org/wiki/Category:8080_Assembly
    let mut vm = execute_vm_with_asm_and_hypervisor("
        ;set fib(n) to calculate
        int 0xAB ;get value from hypervisor
        mov EAX, EBX
        
        fib:
        cmp eax, 0 
        je fib0 
        cmp eax, 1
        je fib1
        mov ECX, EAX
        dec ECX
        mov EAX, 1
        mov EBX, 0
        compute:
        mov EDX, EAX
        add EAX, EBX
        mov EBX, EDX
        dec ECX
        jnz compute

        jmp end_fib
        
        fib0:
        mov eax, 0
        jmp end_fib
        fib1:
        mov eax, 1

        end_fib:
        nop
        hlt

    ", &mut hv);
    assert_eq!(vm.reg32(Reg32::EAX), 1);
    reset_vm(&mut vm);
    execute_vm_with_diagnostics_and_hypervisor(&mut vm, &mut hv);
    assert_eq!(vm.reg32(Reg32::EAX), 0);
    reset_vm(&mut vm);
    execute_vm_with_diagnostics_and_hypervisor(&mut vm, &mut hv);
    assert_eq!(vm.reg32(Reg32::EAX), 1);
    reset_vm(&mut vm);
    execute_vm_with_diagnostics_and_hypervisor(&mut vm, &mut hv);
    assert_eq!(vm.reg32(Reg32::EAX), 55);
}

#[test]
pub fn fibonacci_16bit(){
    let mut hv = TestHypervisor::default();
    hv.pushed_values.push(10);
    hv.pushed_values.push(1);
    hv.pushed_values.push(0);
    hv.pushed_values.push(2);
    //based on https://rosettacode.org/wiki/Category:8080_Assembly
    let mut vm = execute_vm_with_asm_and_hypervisor("
        ;set fib(n) to calculate
        int 0xAB ;get value from hypervisor
        mov EAX, EBX
        
        fib:
        cmp ax, 0 
        je fib0 
        cmp ax, 1
        je fib1
        mov CX, AX
        dec CX
        mov AX, 1
        mov BX, 0
        compute:
        mov DX, AX
        add AX, BX
        mov BX, DX
        dec CX
        jnz compute

        jmp end_fib
        
        fib0:
        mov ax, 0
        jmp end_fib
        fib1:
        mov ax, 1

        end_fib:
        nop
        hlt

    ", &mut hv);
    assert_eq!(vm.reg32(Reg32::EAX), 1);
    reset_vm(&mut vm);
    execute_vm_with_diagnostics_and_hypervisor(&mut vm, &mut hv);
    assert_eq!(vm.reg32(Reg32::EAX), 0);
    reset_vm(&mut vm);
    execute_vm_with_diagnostics_and_hypervisor(&mut vm, &mut hv);
    assert_eq!(vm.reg32(Reg32::EAX), 1);
    reset_vm(&mut vm);
    execute_vm_with_diagnostics_and_hypervisor(&mut vm, &mut hv);
    assert_eq!(vm.reg32(Reg32::EAX), 55);
}