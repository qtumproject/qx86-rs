extern crate qx86;

use qx86::vm::*;

pub const CODE_MEM:u32 = 0x10000;
pub const DATA_MEM:u32 = 0x80000000;

pub fn create_vm() -> VM{
    let mut vm = VM::default();
    vm.eip = CODE_MEM;
    vm.memory.add_memory(CODE_MEM, 0x1000).unwrap();
    vm.memory.add_memory(DATA_MEM, 0x1000).unwrap();
    vm
}