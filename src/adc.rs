//! Routines for interacting with the HMCAD1511 ADC
//! No support for the HMADC1520
use packed_struct::prelude::*;

// As far as I can tell, we talk to the ADC over "Wishbone".
// This is exposed to us via more Katcp messages, specifically
// "write_int" and "read_int" and "read". So, here we abstract
// reading and writing from the ADC's registers by wrapping those
// in nice rust data structures, and then doing the serde with katcp

#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum QuadChannel {
    Ch1 = 0,
    Ch2 = 1,
    Ch3 = 2,
    Ch4 = 3,
}
