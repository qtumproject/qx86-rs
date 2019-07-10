extern crate qx86;
mod common;

use qx86::vm::*;
use common::*;

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
    let bytes = vec![
        0x90, //nop
        0xF4 //hlt
    ];
    vm.copy_into_memory(CODE_MEM, &bytes).unwrap();
    assert!(vm.execute().unwrap());
    assert_eq!(vm.eip, CODE_MEM + 1);
}



