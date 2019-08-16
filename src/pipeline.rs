use crate::opcodes::*;
use crate::structs::*;
use crate::vm::*;
use crate::decoding::*;

#[allow(dead_code)] //remove after design stuff is done

#[derive(Default)]
struct PrefixesActivated {
    size_override: bool,
    two_bytes: bool
}

impl PrefixesActivated {
    fn get_prefixes(&mut self, buffer: &[u8], prefix_size: u8) -> Result<u8, VMError> {
        match buffer[0]{
            0x66 => {
                self.size_override = true;
                return self.get_prefixes(&buffer[1..], prefix_size+1);
            },
            0x0F => {
                self.two_bytes = true;
                Ok(prefix_size+1)
            },
            _ => {
                Ok(prefix_size)
            }
        }
    }
}

/// This is a single execution unit of a pipeline
/// This includes all decoded information that the logic of an opcode would need to execute
#[derive(Copy, Clone)]
pub struct Pipeline{
    /// The function pointer to the opcode logic function
    pub function: OpcodeFn,
    /// The decoded arguments to be sent to the opcode logic function
    pub args: [OpArgument; MAX_ARGS],
    /// The gas cost of the current operation
    pub gas_cost: u64,
    /// The size of the current opcode
    pub eip_size: u8,
    /// Set to true if an operand size override prefix is present
    pub size_override: bool
}

impl Default for Pipeline{
    fn default() -> Pipeline {
        Pipeline{
            function: nop,
            args: [OpArgument::default(), OpArgument::default(), OpArgument::default()],
            gas_cost: 0,
            eip_size: 1,
            size_override: false
        }
    }
}

/// Clears the pipeline. 
/// Note the pipeline is expected to be of fixed size and to not incur any allocation within the main loop of the VM
pub fn clear_pipeline(pipeline: &mut [Pipeline]){
    for _n in 0..pipeline.len(){
        pipeline[0] = Pipeline::default();
    }
}

/// Decode the stream of opcodes and fill the pipeline with decoded opcodes for later execution
/// Note the pipeline is expected to be of fixed size and to not incur any allocation within the main loop of the VM
pub fn fill_pipeline(vm: &VM, opcodes: &[OpcodeProperties], pipeline: &mut [Pipeline]) -> Result<(), VMError>{
    let mut eip = vm.eip;
    let mut stop_filling = false;
    let mut running_gas = vm.gas_remaining;
    //writeable if in memory with top bit set
    let writeable = vm.eip & 0x80000000 > 0;
    clear_pipeline(pipeline);
    for n in 0..pipeline.len(){
        let mut p = &mut pipeline[n];
        p.gas_cost = 0; //this can be reused, so make sure to clear previous
        if stop_filling {
            p.function = nop;
            p.eip_size = 0;
            p.gas_cost = 0;
        }else{
            let mut buffer = vm.memory.get_sized_memory(eip, 16)?;
            let mut prefixes = PrefixesActivated::default();
            let prefix_size = prefixes.get_prefixes(buffer, 0)?;
            buffer = &buffer[prefix_size as usize..];
            let prop = &opcodes[buffer[0 as usize] as usize] | (prefixes.two_byte << 8);
            // 
            // opcode = buffer[0] | prefixes.two_bytes << 8;

            let mut modrm = Option::None;
            let opcode = if prop.has_modrm{
                p.gas_cost += vm.charger.cost(GasCost::ModRMSurcharge);
                modrm = Some(ParsedModRM::from_bytes(buffer)?);
                &prop.opcodes[modrm.unwrap().modrm.reg as usize]
            }else{
                &prop.opcodes[0]
            };
            p.function = opcode.function;
            p.gas_cost += vm.charger.cost(opcode.gas_cost);
            p.size_override = prefixes.size_override;
            match opcode.pipeline_behavior{
                PipelineBehavior::None => {
                    p.eip_size = decode_args_with_modrm(opcode, buffer, &mut p.args, prefixes.size_override, false, modrm)? as u8 + prefix_size;
                },
                PipelineBehavior::Unpredictable | PipelineBehavior::UnpredictableNoGas => {
                    p.eip_size = decode_args_with_modrm(opcode, buffer, &mut p.args, prefixes.size_override, false, modrm)? as u8 + prefix_size;
                    eip += p.eip_size as u32;
                    stop_filling = true;
                },
                PipelineBehavior::RelativeJump => {
                    p.eip_size = decode_args_with_modrm(opcode, buffer, &mut p.args, prefixes.size_override, false, modrm)? as u8 + prefix_size;
                    //relative jumps are calculated from the EIP value AFTER the jump would've executed, ie, after EIP is advanced by the size of the instruction
                    let future_eip = eip + (p.eip_size as u32);
                    //rel must be sign extended, but is otherwise treated as a u32 for simplicity
                    //an i32 and a u32 will behave the same way for wrapping_addition like this
                    let rel = vm.get_arg(p.args[0].location)?.u32_sx()?;
                    //subtract out the eip_size that'll be advanced in the main loop
                    eip = future_eip.wrapping_add(rel) - (p.eip_size as u32);
                    if p.size_override{
                        return Err(VMError::ReadBadMemory(eip & 0xFFFF));
                    }
                }
            };
            p.gas_cost += match opcode.pipeline_behavior{
                PipelineBehavior::Unpredictable => vm.charger.cost(GasCost::ConditionalBranch),
                _ => 0
            };
            for i in 0..MAX_ARGS{
                p.gas_cost += if p.args[i].is_memory{
                    vm.charger.cost(GasCost::MemoryAccess)
                }else{
                    0
                };
            }
            eip += p.eip_size as u32;
        }
        if writeable {
            //if in writeable space, only use one pipeline slot at a time
            //otherwise, the memory we are decoding could be changed by an opcode within the pipeline
            p.gas_cost += vm.charger.cost(GasCost::WriteableMemoryExec);
            stop_filling = true;
        }
        running_gas = running_gas.saturating_sub(p.gas_cost);
        stop_filling |= running_gas == 0;
    }
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;

    //just a simple test function for comparison
    fn test_op(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{Ok(())}
    fn test2_op(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{Ok(())}
    fn test3_op(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{Ok(())}
    fn test4_op(_vm: &mut VM, _pipeline: &Pipeline) -> Result<(), VMError>{Ok(())}

    /* Opcodes defined:
    0x00 -- undefined (purposefully)
    0x01 (), 10 gas -- test_op
    0x02 (imm8), 2 gas -- nop
    0x03 (imm32), 50 gas -- test3_op, conditional jump behavior
    0x10 + r (reg32, off32), 23 gas -- test2_op
    */
    fn test_opcodes() -> [OpcodeProperties; OPCODE_TABLE_SIZE]{
        use OpcodeValueSize::*;
        use ValueSize::*;
        let mut table = [OpcodeProperties::default(); OPCODE_TABLE_SIZE];

        define_opcode(0x01)
            .with_gas(GasCost::Low)
            .calls(test_op)
            .into_table(&mut table);
        define_opcode(0x02)
            .with_arg(ArgSource::ImmediateValue, Fixed(Byte))
            .with_gas(GasCost::VeryLow)
            .calls(nop)
            .into_table(&mut table);
        define_opcode(0x03)
            .with_arg(ArgSource::JumpRel, Fixed(Dword))
            .is_unpredictable()
            .with_gas(GasCost::High)
            .calls(test3_op)
            .into_table(&mut table);
        define_opcode(0x10)
            .with_arg(ArgSource::RegisterSuffix, Fixed(Dword))
            .with_arg(ArgSource::ImmediateAddress, Fixed(Dword))
            .with_gas(GasCost::Moderate)
            .calls(test2_op)
            .into_table(&mut table);
        define_opcode(0xFF)
            .is_group(3)
            .with_rmw()
            .calls(test4_op)
            .into_table(&mut table);

        table
    }

    #[test]
    fn test_simple_pipeline(){
        let opcodes = test_opcodes();
        let mut vm = VM::default();
        vm.gas_remaining = 1;
        let vm_mem = vm.memory.add_memory(0x10000, 0x100).unwrap();
        vm.eip = 0x10000;
        let bytes = vec![
            0x01, //test_op
            0x02, 0x15, //nop, imm8
            0x12, 0x11, 0x22, 0x33, 0x44, //test2_op, EDX, off32
        ];
        (&mut vm_mem[0..bytes.len()]).copy_from_slice(&bytes);
        let mut pipeline = vec![];
        pipeline.resize(2, Pipeline::default());
        fill_pipeline(&vm, &opcodes, &mut pipeline).unwrap();
        
        //Function pointers must be cast to usize here
        //This may break later, so do NOT do in production code
        //See also: https://www.reddit.com/r/rust/comments/98xlh3/how_can_i_compare_two_function_pointers_to_see_if/
        //If weird things break here later, it might be worth figuring out if this is the reason

        assert!(pipeline[0].function as usize == test_op as usize);
        assert!(pipeline[0].args[0].location == ArgLocation::None);
        assert!(pipeline[0].eip_size == 1);

        assert!(pipeline[1].function as usize == nop as usize);
        assert!(pipeline[1].args[0].location == ArgLocation::Immediate(SizedValue::Byte(0x15)));
        assert!(pipeline[1].args[1].location == ArgLocation::None);
        assert!(pipeline[1].eip_size == 2);

        vm.eip += pipeline[0].eip_size as u32 + pipeline[1].eip_size as u32;
        fill_pipeline(&vm, &opcodes, &mut pipeline).unwrap();

        assert_eq!(pipeline[0].function as usize, test2_op as usize);
        assert!(pipeline[0].args[0].location == ArgLocation::RegisterValue(2, ValueSize::Dword));
        assert!(pipeline[0].args[1].location == ArgLocation::Address(0x44332211, ValueSize::Dword));
        assert!(pipeline[0].eip_size == 5); 
    }

    #[test]
    fn test_cond_jump_pipeline(){
        let opcodes = test_opcodes();
        let mut vm = VM::default();
        vm.gas_remaining = 1;
        let vm_mem = vm.memory.add_memory(0x10000, 0x100).unwrap();
        vm.eip = 0x10000;
        let bytes = vec![
            0x01, //test_op
            0x03, 0x11, 0x22, 0x33, 0x44, //test3_op, imm32, cond jump
            0x12, 0x11, 0x22, 0x33, 0x44, //test2_op, EDX, off32
        ];
        (&mut vm_mem[0..bytes.len()]).copy_from_slice(&bytes);
        let mut pipeline = vec![];
        pipeline.resize(3, Pipeline::default());
        fill_pipeline(&vm, &opcodes, &mut pipeline).unwrap();

        assert!(pipeline[0].function as usize == test_op as usize);
        assert!(pipeline[0].args[0].location == ArgLocation::None);
        assert!(pipeline[0].eip_size == 1);

        assert!(pipeline[1].function as usize == test3_op as usize);
        assert!(pipeline[1].args[0].location == ArgLocation::Immediate(SizedValue::Dword(0x44332211)));
        assert!(pipeline[1].args[1].location == ArgLocation::None);
        assert!(pipeline[1].eip_size == 5); 

        //ensure next opcode after conditional jump is nop
        assert!(pipeline[2].function as usize == nop as usize);
        assert!(pipeline[2].args[0].location == ArgLocation::None);
        assert!(pipeline[2].args[1].location == ArgLocation::None);
        assert!(pipeline[2].eip_size == 0);  
        //assert!(pipeline[2].gas_cost == 0);  
    }
    #[test]
    fn test_group_opcodes(){
        let opcodes = test_opcodes();
        let mut vm = VM::default();
        vm.gas_remaining = 1;
        let vm_mem = vm.memory.add_memory(0x10000, 0x100).unwrap();
        vm.eip = 0x10000;
        let bytes = vec![
            0xFF, 0x1A, //test4_op /3 [EDX]
        ];
        (&mut vm_mem[0..bytes.len()]).copy_from_slice(&bytes);
        let mut pipeline = vec![];
        pipeline.resize(3, Pipeline::default());
        fill_pipeline(&vm, &opcodes, &mut pipeline).unwrap();

        assert_eq!(pipeline[0].function as usize, test4_op as usize);
        assert_eq!(pipeline[0].args[0].location, ArgLocation::RegisterAddress(Reg32::EDX as u8, ValueSize::Dword));
        assert_eq!(pipeline[0].eip_size, 2);
    }
}



