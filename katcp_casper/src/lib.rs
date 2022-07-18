use base64::{decode, encode};
use katcp::prelude::*;
use katcp_derive::{KatcpDiscrete, KatcpMessage};

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
/// Request information about the available named registers
/// Always requests the size information
pub enum Listdev {
    Request,
    Inform { register: String },
    Reply { ret_code: RetCode },
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
/// Lists the gateware images stored on the device
pub enum Listbof {
    Request,
    Reply(IntReply),
    Inform { filename: String },
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
/// Programs the FPGA with a BOF file that exists on the device
pub enum Progdev {
    Request { filename: String },
    Reply { ret_code: RetCode },
}

#[derive(KatcpDiscrete, Debug, PartialEq, Eq, Clone)]
pub enum FpgaStatus {
    Loaded,
    Ready,
    Down,
    Mapped,
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
/// Infrom messages we get from certain FPGA commands
pub enum Fpga {
    Inform { status: FpgaStatus },
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
/// Opens a port on the server to allow us to upload a BOF or FPG file
pub enum Progremote {
    Request { port: u32 },
    Reply { ret_code: RetCode },
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
/// Requests the SNAP's status, sending some useful log messages along the way
pub enum Fpgastatus {
    Request,
    Reply { ret_code: RetCode },
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
pub enum Read {
    Request {
        name: String,
        offset: u32,
        num_bytes: u32,
    },
    Reply {
        ret_code: RetCode,
        bytes: Base64Bytes,
    },
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
pub enum Write {
    Request {
        name: String,
        offset: u32,
        bytes: Base64Bytes,
    },
    Reply {
        ret_code: RetCode,
    },
}

// We need explicit serde for hex literals
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Base64Bytes(pub Vec<u8>);

impl ToKatcpArgument for Base64Bytes {
    fn to_argument(&self) -> String {
        encode(&self.0)
    }
}

impl FromKatcpArgument for Base64Bytes {
    type Err = KatcpError;

    fn from_argument(s: impl AsRef<str>) -> Result<Self, Self::Err> {
        decode(s.as_ref())
            .map_err(|_| KatcpError::BadArgument)
            .map(Base64Bytes)
    }
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
pub enum Version {
    Inform { hash: String },
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
pub enum BuildState {
    Inform { timestamp: String },
}

#[cfg(test)]
mod tests {
    use katcp::messages::common::roundtrip_test;

    use super::*;

    #[test]
    fn test_listbof() {
        roundtrip_test(Listbof::Request);
        roundtrip_test(Listbof::Reply(IntReply::Ok { num: 12 }));
        roundtrip_test(Listbof::Inform {
            filename: "dsa_10gv11.bof".to_owned(),
        });
    }

    #[test]
    fn test_read() {
        roundtrip_test(Read::Request {
            name: "gbe0".to_owned(),
            offset: 0,
            num_bytes: 4,
        });
        roundtrip_test(Read::Reply {
            ret_code: RetCode::Ok,
            bytes: Base64Bytes(vec![0xde, 0xad, 0xbe, 0xef]),
        });
    }
}
