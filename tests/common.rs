extern crate qx86;
extern crate tempfile;

use qx86::vm::*;
use qx86::structs::*;
use qx86::decoding::*;

pub const CODE_MEM:u32 = 0x10000;
pub const DATA_MEM:u32 = 0x80000000;
pub const INITIAL_GAS:u64 = 10000000;

pub fn create_vm() -> VM{
    let mut vm = VM::default();
    vm.eip = CODE_MEM;
    vm.charger = GasCharger::test_schedule();
    vm.gas_remaining = INITIAL_GAS;
    vm.memory.add_memory(CODE_MEM, 0x10000).unwrap();
    vm.memory.add_memory(DATA_MEM, 0x10000).unwrap();
    vm
}

pub fn create_vm_with_asm(input: &str) -> VM{
    let mut vm = create_vm();
    let bytes = asm(input);
    vm.copy_into_memory(CODE_MEM, &bytes).unwrap();
    vm
}

pub fn execute_vm_with_asm(input: &str) -> VM{
    let mut vm = create_vm_with_asm(input);
    execute_vm_with_diagnostics(&mut vm);
    vm
}
pub fn execute_vm_with_diagnostics(vm: &mut VM){
    let r = vm.execute();
    if r.is_err(){
        println!("VM returned error: {}", r.unwrap_err());
        println!("EAX: 0x{:08X?}", vm.reg32(Reg32::EAX));
        println!("ECX: 0x{:08X?}", vm.reg32(Reg32::ECX));
        println!("EDX: 0x{:08X?}", vm.reg32(Reg32::EDX));
        println!("EBX: 0x{:08X?}", vm.reg32(Reg32::EBX));
        println!("ESP: 0x{:08X?}", vm.reg32(Reg32::ESP));
        println!("EBP: 0x{:08X?}", vm.reg32(Reg32::EBP));
        println!("ESI: 0x{:08X?}", vm.reg32(Reg32::ESI));
        println!("EDI: 0x{:08X?}", vm.reg32(Reg32::EDI));
        println!();
        println!("Gas remaining: {}", vm.gas_remaining);
        println!("EIP: 0x{:X?}", vm.eip);
        println!("Surrounding bytes in opcode stream:");
        for n in std::cmp::max(vm.eip - 8, 0x10000)..(vm.eip + 8){
            let b = vm.get_mem(n, ValueSize::Byte).unwrap().u8_exact().unwrap();
            println!("0x{:X?}: 0x{:02X}, as modrm: {}, as sib: {}", n, b, ModRM::parse(b), SIB::parse(b));
        }
    }
    r.unwrap();
}

pub fn asm(input: &str) -> Vec<u8>{
    use tempfile::*;
    use std::io::Write;
    use std::process::Command;
    use std::io::Read;
    let asm = format!("{}{}", "
[bits 32]
[org 0x10000]
[CPU i686]
", input);
    let dir = tempdir().unwrap();
    let input = dir.path().join("test_code.asm");
    //println!("input: {}", input.to_str().unwrap());
    println!("asm: {}\n---------------", asm);
    let output = dir.path().join("test_code.asm.bin");
    {
        let mut f = std::fs::File::create(&input).unwrap();
        writeln!(f,"{}", asm).unwrap();
        f.flush().unwrap();
    
        let output = Command::new("yasm").
            arg("-fbin").
            arg(format!("{}{}", "-o", &output.to_str().unwrap())).
            arg(&input).
            output().unwrap();
        println!("yasm stdout: {}", std::str::from_utf8(&output.stdout).unwrap());
        println!("yasm stderr: {}", std::str::from_utf8(&output.stderr).unwrap());
    }
    let mut v = vec![];
    {
        let mut compiled = std::fs::File::open(output).unwrap();
        compiled.read_to_end(&mut v).unwrap();
    }
    v
}
