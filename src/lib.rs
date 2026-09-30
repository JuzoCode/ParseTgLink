#![cfg_attr(not(test), no_std)]
#![allow(dead_code)]
#![allow(unsafe_op_in_unsafe_fn)]

pub mod parser;

pub use parser::{LinkKind, ParseTgLink};
