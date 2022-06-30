pub(crate) trait RegisterAddress {
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
