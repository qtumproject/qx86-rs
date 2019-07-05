This is just a simple set of thoughts written when considering how to properly implement this VM. The primary goals of the VM being:

* Not overly complicated or time consuming to implement
* Portable and work reasonably well on any platform
* Reasonably effecient, from the beginning (ie, no rewriting to improve performance)


Notes follow:

The x86 VM will seek to use portable (ie, works in Rust without unsafe) designs, while also optimizing as much as possible, so long as the optimizations are not overly time consuming. 

Differing from x86Lib which uses a single decode/execute phase based on function pointer tables, rust-qx86 will use 2 distinct phases:

1. Decode instruction and prepare into execution pipeline
2. Clear and execute the pipeline

Instruction decoding for x86 is far from simple, but by keeping two clear phases, it will be possible to avoid the typically very high rates of branch misprediction caused by using a naive function table, while also avoiding duplicate logic in, for example, mov_rm32_r32 and mov_rm32_imm32. Ideally, there would instead be a simple function which encodes the "assign input to output" logic, and the remaining "input is this, output is that" logic should instead be done ahead of time, in the common decoding. While greatly simplifying implementation, this also decreases overall code size needed by the processor (ie, "icache") and keeps with an easier to predict indirect branching structure. Basically, by structuring our code in a certain way, we get both easier to write and test code, separated concerns, and ideally will execute significantly faster on processors with some level of indirect branch prediction. 


The pipeline will be terminated by the following conditions:

* Dynamic Jumps (ie, conditional or indirect)
* Execution of code within modifiable memory space (in modifiable memory space only 1 instruction can be executed at a time without a lot of additional logic.. so rather, just discourage this)
* An arbritrary max size (to prevent large amounts of processing when an out-of-gas condition could happen)

The pipeline will consist of multiple items of this structure:

* Function pointer to opcode
* Opcode (slice of entire opcode in memory)
* Inputs used
* Outputs used
* Arg 1 (as a complex hellscape structure due to SIB bytes) 
* Arg 2
* Arg 3
* Gas Cost (opcode base cost + decoding cost) 
* Mod/RM extra (used for /r arguments?? and potentially group opcodes)


Hellscape structure:

* location type (immediate address, immediate value, register address, register value)
* location
* base (as a register or 0)
* scale (as a value; 1, 2, 4, or 8)
* index (as a register or 0)
* size (00 for normal address/size, 01 for normal address/override size, 10 for override address/normal size, 11 for override address/override size)

Input/Output storage calculated as: (for address) `location + (index * scale + base)` 

Examples that can all be encoded with a single opcode, `PUSH MODR/M32`:

* `push eax`
* `push [eax]`
* `push [1000]`
* `push [1000 + eax]`
* `push [1000 + eax * 2 + ecx]`

In order for a single "push" logic function to execute each of these variants, there must be some standardized way to represent all of these possibilities into a single input or output argument


Instruction Info:

Index: Opcode number + group number + 0x0F flag + size override flag

* Is group
* Opcode functions [0-7] -- 0 is for non-groups, 0-7 is for group opcodes
* argument types (as bitfield) (size of values)
* Argument source (as bitfield) -- 00 if register in opcode, 01 if immediate, 10 if immediate address, 11 if mod/rm
* base gas cost
* valid with REP
* valid with size override
* valid with address override
* jump behavior (00 = none, 01 = absolute, 10 = relative, 11 = conditional) 

Duplicated tables for 0x0F (two byte) prefix and for operand size prefix

Total size: 8 * 256 * 2 = 4096 -- 4096 * 16 = 64K * 2 = 128K

This table will be internally represented as a 13 bit indexed table with the following bytefield in the 12 bits:

* opcode number - 8 bits
* Mod R/M "extra" field -- 3 bits (used for group opcodes)
* 0x0F extended flag - 1 bit
* operand size override flag -- 1 bit

## Limitations

This VM will not be designed as generalized, and as such we can impose many assumptions and restrictions to simplify implementation as well as to allow for optimizations:

* The top bit of an address is set if accessing modifiable memory (ie, anything >2Gb)
* The second-to-top bit of an address is set if referring to a register (thus, reducing addressable memory to 31 bits). If this bit is set in a memory access, such as `mov eax, [b0100...]`, it will always trigger a memory error in decoding. -- 32 bit registers are 0-7, 16 bit is 8-15, 8 bit is 16-23. The EIP register is referenced internally as the 24th register. Null (including segment immediates) segment registers are referenced as >64
* Segment registers are never used, and accessing them will always return a result of 0. Setting the segment register will always result in an error
* If an opcode crosses a 64k boundary or is less than 8 bytes from the boundary, such as having a 4 byte long opcode at 0x1FFFE or a 4 byte long opcode at 0x1FFFA, a memory error will be triggered. This greatly reduces the complexity of memory access in the opcode decoding stage. In Qtum-x86 it is planned that all memory areas will be at most 64Kbytes large. 
* Segment overrides for SS/ES/GS/FS/CS are nops and there is never a difference in segments. This includes allowing something like writing into memory using the CS segment, which would normally always cause an error on an actual x86 machine
* Only the AF, CF, ZF, PF, and SF flags can be changed. All other flags are always cleared
* PUSH and POP operations for segment registers modify the stack, but otherwise do nothing (equivalent to `push 0; pop [null]` )
* Certain invalid opcode combinations, such as segment prefixes before opcodes which do not access memory are valid, as the segment prefix would be ignored. 
* External interrupt behavior can be completely ignored, because there are no external interrupts. This, for instance, greatly simplifies the behavior of REP


The execution logic will be a series of separate functions for an overall "type" of logic. For instance, with separate functions for `mov`, `sub`, and `mul`. For complex opcodes, it may be broken down further into size-specific functions, such as `sub_8bit` and `sub_32bit` 



The most complicated piece of x86 to decode is the Mod R/M opcode argument. This has some special cases depending on which opcode it is being used with, and is the primary reason that the size of an x86 opcode can not simply be looked up by the first byte of an opcode. It encodes options for indirect memory with simple calculations (such as `[ebx * 2 + eax]`), to immediate constants, to immediate addresses, and of course, also registers. For instance, this is the code used in x86Lib for determining the length of a Mod R/M argument:

    uint8_t ModRM::GetLength(){ //This returns how many total bytes the modrm block consumes
        if(this_cpu->Use32BitAddress()){
            if((modrm.mod==0) && (modrm.rm==5)){
                return 5;
            }
            if(modrm.mod == 3){
                return 1;
            }
            int count=1; //1 for modrm byte
            if(modrm.rm == 4){
                count++; //SIB byte
            }
            switch(modrm.mod){
                case 0:
                    count += 0;
                    break;
                case 1:
                    count += 1;
                    break;
                case 2:
                    count += 4;
                    break;
            }
            return count;
        }else{
            if((modrm.mod==0) && (modrm.rm==6)){
                return 3;
            }
            switch(modrm.mod){
                case 0:
                    return 1;
                    break;
                case 1:
                    return 2;
                    break;
                case 2:
                    return 3;
                    break;
                case 3:
                    return 1;
                    break;
            }
        }
        return 1; //should never reach here, but to avoid warnings...
    }



Other complications:

The REP/REPE/REPNE prefixes basically must be treated as conditional jumps, equivalent to:

    loop:
    [string opcode]
    dec ecx
    je loop (or other condition)

However, the ability to predict the exact case for this, especially in the case of plain REP, is not difficult. 
Thus, discrete string instructions will be implemented for use without a REP prefix, but otherwise REP will use specialized "rep_movs" etc functions in the pipeline. These functions will then not require a termination of the pipeline. This creates large instructions which will be required to assess gas costs before execution of the required function. 

Because there are only 5 string instructions valid in this VM, it should be trivial to to account for this special case in the encoding and not require some specialized function table (like the 0x0F two byte prefix) for these opcodes. 


Error conditions:

Upon error, instead of putting a conditional branch within the

    foreach item in pipeline{
        if item() has error ... 
    }

The pipeline will be cleared after the errored opcode.. To look more like:

    foreach item in pipeline{
        item()
    }

With the remaining pipeline items being an error function which will be cheap to execute, and once the pipeline end has been reached, termination of the VM can commence. 

Crucially, this can be unrolled, to look more like:

    pipeline[0]()
    pipeline[1]()
    pipeline[2]()
    ....

The "termination" of the pipeline will rather look like a series of nop functions until the pipeline is ended

Alternatively, the pipeline could be left alone completely once in execution, and a simple error flag set and some info logged when an error does occur. Then, execution resumes despite it being incorrect. When the execution terminates and goes back to decoding, the flag will be checked and the error handled as if the latter execution never happened. The error flag would also be checked inside of especially expensive instructions, such as a REP opcode or an INT to conduct a system call. The downside of this would be that the state of memory would be incorrect once the error is finally recognized, potentially harming debugging abilities. It is already expected however to have a separate debug version of this VM with a constant pipeline size of 1 to allow for breakpoints etc

Constant jumps

Constant calls are very common in most code, and jmps are somewhat common. The decoder, armed with a bit of knowledge about the type of jump it is dealing with, can automatically follow the jump to a new EIP value and continue building the pipeline from there. Note that ret can not be followed, as it depends on the state of the stack. 

Ideal pipeline value: 

Seems to be some value ranging from 16 to 32, looking at production code

Future Additions:

For this, compatibility with raspberry pi and other potential platforms is essential. However, once the decoding framework is built, it is quite simple to convert pipelines into pieces of JIT code. It is unknown how much of a performance increase this would give though without a much more complex decoding stage capable of following conditional branches and especially indirect branches, such as the common RET opcode


Rationale:

The overall rationale of this VM design is to allow for optimal performance, while remaining portable to various platforms. In addition, the implementation of each opcode should not be overly complicated to consume too much time. Memory is regarded as cheap, so long as the VM uses less than a few Mb for internal structures and code. This VM should also not be overly slow on older or less powerful platforms, such as that on the Raspberry Pi, or on older Intel processors. In addition, the code should be fairly simple to understand without too many low level tricks and easy to expand later as needed, such as for a partial JIT implementation or additional instructions. 





