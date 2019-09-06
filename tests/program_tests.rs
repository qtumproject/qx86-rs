extern crate qx86;
mod common;

use qx86::vm::*;
use common::*;


#[test]
pub fn fibonacci(){
    let mut hv = TestHypervisor::default();
    //based on https://rosettacode.org/wiki/Category:8080_Assembly
    let vm = execute_vm_with_asm_and_hypervisor("
        ;set fib(n) to calculate
        mov EAX, 10
        
        fib:
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
        
        hlt
    ", &mut hv);
    assert_eq!(vm.reg32(Reg32::EBX), 34);
}
