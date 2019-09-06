extern crate qx86;
mod common;

use qx86::vm::*;
use common::*;

fn cost_from_list(charger: &GasCharger, costs: &[GasCost]) -> u64{
    let mut cost = 0;
    for c in costs{
        cost += charger.cost(*c);
    }
    cost
}


#[test]
fn test_hlt_gas(){
    let vm = execute_vm_with_asm("
        hlt");
    //this should actually consume no gas
    assert_eq!(vm.gas_remaining, INITIAL_GAS);
}

#[test]
fn test_simple_gas(){
    use GasCost::*;
    //mov is VeryLow
    let vm = execute_vm_with_asm("
        mov eax, 0x80000000 ;VeryLow
        mov ecx, [eax] ;VeryLow + Memory + ModRM
        nop ;None
        hlt ;None
        ");
    assert_eq!(vm.gas_remaining, INITIAL_GAS - cost_from_list(&vm.charger, &[VeryLow, VeryLow, MemoryAccess, ModRMSurcharge]));
}

//Need more tests here once more opcodes are implemented, especially jmp and jcc

#[test]
fn test_perfect_gas_amount(){
    use GasCost::*;
    let mut vm = create_vm_with_asm("
        mov eax, 0x80000000 ;VeryLow
        mov ecx, [eax] ;VeryLow + Memory + ModRM
        nop ;None
        hlt ;None
        ");
    vm.gas_remaining = cost_from_list(&vm.charger, &[VeryLow, VeryLow, MemoryAccess, ModRMSurcharge]);
    let mut hv = TestHypervisor::default();
    vm.execute(&mut hv).unwrap();
}

#[test]
fn test_out_of_gas(){
    use GasCost::*;
    let mut vm = create_vm_with_asm("
        mov eax, 0x80000000 ;VeryLow -- size: 5
        mov ecx, [eax] ;VeryLow + Memory + ModRM -- size: 2
        nop ;None -- size: 1
        hlt ;None -- size: 1
        ");
    vm.gas_remaining = cost_from_list(&vm.charger, &[VeryLow, VeryLow, MemoryAccess, ModRMSurcharge]) - 1;
    let mut hv = TestHypervisor::default();
    let r = vm.execute(&mut hv);
    assert_eq!(r.err().unwrap(), VMError::OutOfGas);
    //should stop at the `mov ecx, [eax]`
    assert_eq!(vm.eip, CODE_MEM + 5); 
}