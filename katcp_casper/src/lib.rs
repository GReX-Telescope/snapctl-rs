use hex;
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
pub enum Wordread {
    Request { name: String, offset: u32 },
    Reply { ret_code: RetCode, word: HexWord },
}

// We need explicit serde for hex literals
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct HexWord(pub u32);

impl ToKatcpArgument for HexWord {
    fn to_argument(&self) -> String {
        format!("0x{}", hex::encode(self.0.to_be_bytes()))
    }
}

impl FromKatcpArgument for HexWord {
    type Err = KatcpError;

    fn from_argument(s: impl AsRef<str>) -> Result<Self, Self::Err> {
        if s.as_ref()[..2] == *"0x" {
            Ok(HexWord(u32::from_be_bytes(
                hex::decode(&s.as_ref()[2..])
                    .map_err(|_| KatcpError::BadArgument)?
                    .try_into()
                    .map_err(|_| KatcpError::BadArgument)?,
            )))
        } else {
            Err(KatcpError::BadArgument)
        }
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
    fn test_hex() {
        roundtrip_test(Wordread::Request {
            name: "gbe0".to_owned(),
            offset: 0,
        });
        roundtrip_test(Wordread::Reply {
            ret_code: RetCode::Ok,
            word: HexWord(42069),
        });
    }
}
