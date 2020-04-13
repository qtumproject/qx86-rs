#![allow(dead_code)] //todo remove later on when this is closer to finished

#[macro_use]
extern crate lazy_static;
extern crate strum;
#[macro_use]
extern crate strum_macros;

/// Helper structures used across the entire VM and sometimes externally
pub mod structs;
/// Instruction and argument decoding supports
pub mod decoding;
/// Opcode definitions including the giant master opcode map for the subset of x86 that qx86 implements
pub mod opcodes;
/// Execution pipeline building supports
pub mod pipeline;
/// Primary VM interface and functions which opcode logic functions can use to modify and retrieve the VM state
pub mod vm;
/// Memory support used by the VM to track virtual mmemory
pub mod memory;
/// The actual opcode logic function implementations
mod ops;
/// The structures used for flag register and flag register calculations
pub mod flags;

mod bitmanip;




