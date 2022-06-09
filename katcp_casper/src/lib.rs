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
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
/// Infrom messages we get from certain FPGA commands
pub enum Fpga {
    Inform { status: FpgaStatus },
}

#[derive(KatcpMessage, Debug, PartialEq, Eq, Clone)]
/// Opens a port on the server to allow us to upload a file
pub enum Upload {
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
    Request {},
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
}
