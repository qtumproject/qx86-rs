extern crate qx86;
mod common;

use qx86::vm::*;
use common::*;


#[test]
fn test_simple_nop_hlt(){
    let mut vm = common::create_vm();
    let bytes = vec![
        0x90,
        0xF4
    ];
    vm.copy_into_memory(CODE_MEM, &bytes).unwrap();
    assert!(vm.execute().unwrap());
    assert_eq!(vm.eip, CODE_MEM + 2);
}



