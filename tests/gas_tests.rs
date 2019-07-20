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
fn test_hlt_usage(){
    let vm = execute_vm_with_asm("
        hlt");
    //this should actually consume no gas
    assert_eq!(vm.gas_remaining, INITIAL_GAS);
}

