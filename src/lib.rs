#![allow(dead_code)] //todo remove later on when this is closer to finished

#[macro_use]
extern crate lazy_static;
extern crate strum;
#[macro_use]
extern crate strum_macros;

pub mod structs;
pub mod decoding;
pub mod opcodes;
pub mod pipeline;
pub mod vm;
pub mod memory;
mod ops;



