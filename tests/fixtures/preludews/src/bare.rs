#![no_implicit_prelude]

use ::std::vec;
use ::std::vec::Vec;

pub fn empty() -> Vec<u32> {
    let mut values = vec![];
    values.push(1u32);
    values
}

pub fn fresh() -> Vec<u32> {
    let mut values = Vec::new();
    values.push(1u32);
    values
}
