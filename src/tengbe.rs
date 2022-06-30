//! Routines for interacting with the CASPER 10GbE Core
use packed_struct::prelude::*;

use crate::{register_address, utils::RegisterAddress};
// The details of the memory map here are magical and come from Jack H

// The 10 GbE Core itself exists as a big register that we can query over katcp
// So, we need to read/write to the register of that name (the name of the block from Simulink)
// at an offset of the address of the thing we care about. We will always read 4 bytes and then
// pass to the packed_struct methods to serde from the rust types

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
enum TenGbeCoreAddress {
    CoreType = 0x0,
    // BufferSizes = 0x4,
    // WordLengths = 0x8,
    // MACAddress = 0xC,
    // IPAddress = 0x14,
    // GatewayAddress = 0x18,
    // Netmask = 0x1C,
    // MulticastIP = 0x20,
    // MulticastMask = 0x24,
    // BytesAvailable = 0x28,
    // PromiscRstEn = 0x2C,
    // Port = 0x30,
    // Status = 0x34,
    // Control = 0x3C,
    // ARPSize = 0x44,
    // TXPacketRate = 0x48,
    // TXPacketCounter = 0x4C,
    // TXValidRate = 0x50,
    // TXValidCounter = 0x54,
    // TXOverflowCounter = 0x58,
    // TXAlmostFullCounter = 0x5C,
    // RXPacketRate = 0x60,
    // RXPacketCounter = 0x64,
    // RXValidRate = 0x68,
    // RXValidCounter = 0x6C,
    // RXOverflowCounter = 0x70,
    // RXBadCounter = 0x74,
    // CounterReset = 0x78,
}

register_address! {TenGbeCoreAddress,CoreType}

#[derive(PackedStruct, Debug)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "4")]
pub struct CoreType {
    #[packed_field(bits = "24")]
    cpu_tx_enable: bool,
    #[packed_field(bits = "16")]
    cpu_rx_enable: bool,
    #[packed_field(bytes = "1")]
    revision: u8,
    #[packed_field(bytes = "0")]
    core_type: u8,
}
