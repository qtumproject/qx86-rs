use crate::opcodes::OpcodeFn;
use crate::structs::*;

#[allow(dead_code)] //remove after design stuff is done


pub struct Pipeline{
    pub function: OpcodeFn,
    pub args: [OpArgument; MAX_ARGS],
    pub gas_cost: i32,
    pub eip_size: u32
}

impl Default for Pipeline{
    fn default() -> Pipeline {
        use crate::opcodes::nop;
        Pipeline{
            function: nop,
            args: [OpArgument::default(), OpArgument::default(), OpArgument::default()],
            gas_cost: 0,
            eip_size: 1
        }
    }
}



fn fill_pipeline(pipeline: &mut [Pipeline]){

}




