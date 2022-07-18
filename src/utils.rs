use std::net::IpAddr;

use crate::Message;
use tokio::{net::tcp::OwnedWriteHalf, sync::mpsc::UnboundedReceiver};

pub trait RegisterAddress {
    /// Returns the address of this particular struct
    fn address() -> u8;
}

/// Auto generates the trait impl from an enum of addresses
#[macro_export]
macro_rules! register_address {
    ($addrs:ident, $reg:ident) => {
        impl RegisterAddress for $reg {
            fn address() -> u8 {
                $addrs::$reg as u8
            }
        }
    };
}

pub struct State {
    pub unhandled_incoming_messages: UnboundedReceiver<Message>,
    // The writer
    pub writer: OwnedWriteHalf,
    // The connection address
    pub address: IpAddr,
}
