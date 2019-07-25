#[macro_use]
extern crate criterion;
extern crate qx86;
extern crate tempfile;

use qx86::vm::*;

use criterion::Criterion;

pub const CODE_MEM:u32 = 0x10000;
pub const DATA_MEM:u32 = 0x80000000;

pub const OOG_GAS_LIMIT:u64 = 10000;

fn run_exec_test(bytecode: &[u8]){
    let mut vm = create_vm();
    vm.copy_into_memory(CODE_MEM, bytecode).unwrap();
    vm.execute().unwrap();
}

fn run_oog_test(bytecode: &[u8]){
    let mut vm = create_vm();
    vm.gas_remaining = OOG_GAS_LIMIT;
    vm.copy_into_memory(CODE_MEM, bytecode).unwrap();
    let r = vm.execute().unwrap_err();
    match r{
        VMError::OutOfGas => return,
        _ => panic!("Unexpected error instead of Out Of Gas")
    };
}

fn nop_hlt_benchmark(c: &mut Criterion) {
    let mut bytes = vec![];
    for _n in 0..2000{
        bytes.push(0x90); //nop
    }
    bytes.push(0xF4); //hlt
    c.bench_function_over_inputs("nop x2000", | i, bytecode | i.iter(|| run_exec_test(bytecode)), vec![bytes]);
}
fn mov_modrm_benchmark(c: &mut Criterion) {
    let bytes = asm("
    mov eax, 0x80000000
    mov edx, 0x10
    %assign i 0
    %rep 1000
        mov dword [edx + 0x80000020], eax
        mov ebx, dword [edx * 2 + eax]
        %assign i i+1
    %endrep
    hlt
    ");
    c.bench_function_over_inputs("mov modrm x2000", | i, bytecode | i.iter(|| run_exec_test(bytecode)), vec![bytes]);
}

fn infinite_loop_oog_benchmark(c: &mut Criterion) {
    let bytes = asm("
    _a:
    jmp _a
    ");
    c.bench_function_over_inputs("oog - infinite loop", | i, bytecode | i.iter(|| run_oog_test(bytecode)), vec![bytes]);
}

fn indirect_infinite_loop_oog_benchmark(c: &mut Criterion) {
    //note this appears to execute faster than the infinite loop benchmark, 
    //but this is due to gas costs being higher for this execution and ending the execution sooner
    let bytes = asm("
    mov eax, _a
    _a:
    jmp eax
    ");
    c.bench_function_over_inputs("oog - indirect infinite loop", | i, bytecode | i.iter(|| run_oog_test(bytecode)), vec![bytes]);
}

criterion_group!(benches, nop_hlt_benchmark, mov_modrm_benchmark, infinite_loop_oog_benchmark, indirect_infinite_loop_oog_benchmark);
criterion_main!(benches);




//Duplicated from test/common.rs
//fix this later.. 
pub fn create_vm() -> VM{
    let mut vm = VM::default();
    vm.eip = CODE_MEM;
    vm.gas_remaining = 1000000000;
    vm.charger = GasCharger::test_schedule();
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

pub fn execute_vm_asm(input: &str) -> VM{
    let mut vm = create_vm_with_asm(input);
    assert!(vm.execute().unwrap());
    vm
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
