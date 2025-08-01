#![no_std]

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SomeData {
    pub counter: u64,
    pub value0: f64,
    pub value1: u32,
    pub value2: u8,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SmallArray {
    pub data: [u8; 32],
}
