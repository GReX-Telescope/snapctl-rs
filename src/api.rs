//! This module holds the top-level functions for interacting with the connected SNAP

use crate::{tengbe::*, utils::*};
use katcp::{
    messages::{core::*, log::*},
    prelude::*,
};
use katcp_casper::*;
use packed_struct::{prelude::PackedStruct, PackingError};
use std::{
    fmt::Debug,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{sleep, Duration},
};
use tracing::{debug, info, trace, warn};

pub async fn make_request<T>(state: &mut State, request: T) -> Result<Vec<T>, String>
where
    T: KatcpMessage + Debug,
    <T as TryFrom<Message>>::Error: Debug,
{
    // Serialize and send request
    let request_msg = request
        .to_message(None)
        .expect("Could not serialize request to a KATCP message");
    if request_msg.kind() != MessageKind::Request {
        return Err("We tried to send a request message that wasn't actually a request".to_owned());
    }
    trace!(?request, "Sending a request");
    state
        .writer
        .write_all(request_msg.to_string().as_bytes())
        .await
        .expect("Error writting bytes to TCP connection");
    let mut messages = vec![];
    loop {
        match state.unhandled_incoming_messages.recv().await {
            Some(v) => match v.kind() {
                MessageKind::Request => unreachable!(),
                MessageKind::Inform => {
                    let msg = v.try_into();
                    match msg {
                        Ok(msg) => messages.push(msg),
                        Err(e) => {
                            debug!(?e, "Unexpected message");
                            continue;
                        }
                    }
                }
                MessageKind::Reply => {
                    let msg = v.try_into().expect("Got a Reply we couldn't deserialize");
                    messages.push(msg);
                    break;
                }
            },
            None => panic!("The channel we were expecting messages from has been closed"),
        }
    }
    Ok(messages)
}

pub async fn ping(state: &mut State) {
    match make_request(state, Watchdog::Request).await {
        Ok(v) => {
            if let Watchdog::Reply(GenericReply::Ok) = v.get(0).unwrap() {
                debug!("Got a successful ping!");
            } else {
                panic!("Got a bad ping, we're bailing");
            }
        }
        Err(e) => {
            println!("{}", e);
            panic!("Ping errored: we're bailing");
        }
    }
}

pub async fn set_device_log_level(state: &mut State, lvl: Level) {
    match make_request(state, LogLevel::Request { level: lvl }).await {
        Ok(v) => {
            if let LogLevel::Reply { ret_code, level } = v.get(0).unwrap() {
                assert_eq!(*ret_code, RetCode::Ok);
                assert_eq!(*level, lvl);
                debug!("Set log level successfully!");
            } else {
                panic!("Got a bad log level response, we're bailing");
            }
        }
        Err(e) => {
            println!("{}", e);
            panic!("Setting log level errored: we're bailing");
        }
    }
}

pub async fn read(register_name: &str, offset: u32, num_bytes: u32, state: &mut State) -> Vec<u8> {
    match make_request(
        state,
        Read::Request {
            name: register_name.to_owned(),
            offset,
            num_bytes,
        },
    )
    .await
    {
        Ok(v) => {
            if let Read::Reply { ret_code, bytes } = v.get(0).unwrap() {
                assert_eq!(*ret_code, RetCode::Ok);
                debug!("Read word successfully!");
                bytes.0.clone()
            } else {
                panic!("Got a bad read response, we're bailing");
            }
        }
        Err(e) => {
            println!("{}", e);
            panic!("Reading bytes errored: we're bailing");
        }
    }
}

async fn read_bool(register_name: &str, state: &mut State) -> bool {
    *read(register_name, 0, 1, state)
        .await
        .get(0)
        .expect("Get 1 byte back from request")
        == 1
}

async fn read_int(register_name: &str, state: &mut State) -> u32 {
    // CASPER registers are big endian
    u32::from_be_bytes(
        read(register_name, 0, 4, state)
            .await
            .try_into()
            .expect("Get 4 bytes back from request"),
    )
}

pub async fn write(register_name: &str, offset: u32, bytes: &[u8], state: &mut State) {
    match make_request(
        state,
        Write::Request {
            name: register_name.to_owned(),
            offset,
            bytes: Base64Bytes(bytes.to_vec()),
        },
    )
    .await
    {
        Ok(v) => {
            if let Write::Reply { ret_code } = v.get(0).unwrap() {
                assert_eq!(*ret_code, RetCode::Ok);
                debug!("Wrote word successfully!");
            } else {
                panic!("Got a bad read response, we're bailing");
            }
        }
        Err(e) => {
            println!("{}", e);
            panic!("Reading bytes errored: we're bailing");
        }
    }
}

async fn write_bool(register_name: &str, v: bool, state: &mut State) {
    write(register_name, 0, &[v as u8], state).await
}

async fn write_int(register_name: &str, v: u32, state: &mut State) {
    // CASPER registers are big endian
    write(register_name, 0, &v.to_be_bytes(), state).await
}

pub async fn read_packed<T, const N: usize>(
    name: &str,
    state: &mut State,
) -> Result<T, PackingError>
where
    T: PackedStruct<ByteArray = [u8; N]> + RegisterAddress,
{
    let bytes = read(name, T::address() as u32, N as u32, state).await;
    T::unpack(&bytes.try_into().expect("We already read N bytes"))
}

pub async fn write_packed<T, const N: usize>(name: &str, packed: T, state: &mut State)
where
    T: PackedStruct<ByteArray = [u8; N]> + RegisterAddress,
{
    write(
        name,
        T::address() as u32,
        &packed
            .pack()
            .expect("An instance of a packed struct should always pack"),
        state,
    )
    .await;
}

//////////////////////////////// Command line subcommands

/// Setups the GbE core for use
pub async fn config_gbe(core: &str, state: &mut State) {
    // Disable all the counters for the duration of the setup
    write_bool("tx_en", false, state).await;
    // Configure the MAC address
    // Configure the IP
    write_packed(core, IpAddress(Ipv4Addr::new(192, 168, 5, 20)), state).await;
    // Configure the Port
    write_packed(
        core,
        Port {
            port_mask: 0,
            port: 6000,
        },
        state,
    )
    .await;
    // Configure the gateway
    write_packed(core, GatewayAddress(Ipv4Addr::new(192, 168, 5, 1)), state).await;
    // Set the destination IP and Port
    write_int("dest_ip", Ipv4Addr::new(192, 168, 5, 1).into(), state).await;
    write_int("dest_port", 6000, state).await;
    // Add the server to the ARP table
    // Set the core's enable
    write_packed(
        core,
        PromiscRstEn {
            soft_rst: false,
            promisc: false,
            enable: true,
        },
        state,
    )
    .await;
    // Toggle the core's reset
    write_packed(
        core,
        PromiscRstEn {
            soft_rst: true,
            promisc: false,
            enable: true,
        },
        state,
    )
    .await;
    write_packed(
        core,
        PromiscRstEn {
            soft_rst: false,
            promisc: false,
            enable: true,
        },
        state,
    )
    .await;
    // Toggle the reset line
    write_bool("tx_rst", true, state).await;
    write_bool("tx_rst", false, state).await;
    // Re-enable
    write_bool("tx_en", true, state).await;
    // Sleep a bit to wait for boot
    sleep(Duration::from_millis(500)).await;
    // Check if link is up
    let status: Status = read_packed(core, state)
        .await
        .expect("State read shouldn't fail");
    if status.link_up {
        info!("10 GbE Link is up");
    } else {
        warn!("10 GbE Link is not up, something might be wrong");
    }
}

/// Uploads and programs the file given by `path` to the FPGA over the upload port `port`
pub async fn upload(path: PathBuf, port: u16, state: &mut State) {
    // Upload the file directly and then try to program
    debug!("The file we want to program doesn't exist on the device (or we're forcing an upload), upload it instead");
    info!("Attempting to program: {}", path.display());
    // Get an upload port
    match make_request(
        state,
        Progremote::Request {
            port: (port as u32),
        },
    )
    .await
    {
        Ok(v) => {
            // We should have gotten one reply
            if let Some(Progremote::Reply { ret_code }) = v.get(0) {
                if *ret_code == RetCode::Ok {
                    debug!("Upload port set: waiting for data");
                }
            } else {
                panic!("Request for upload failed, see logs");
            }
        }
        Err(e) => {
            println!("{}", e);
            panic!("Requesting an upload port failed: we're bailing");
        }
    };
    info!("Uploading {}", path.display());
    // Netcat the file over
    let mut file = File::open(path)
        .await
        .expect("Could not open file. Is the path correct?");
    // Read all the data into a buffer here
    let mut contents = vec![];
    file.read_to_end(&mut contents)
        .await
        .expect("Couldn't read boffile");
    let mut upload_stream = TcpStream::connect(SocketAddr::new(state.address, port))
        .await
        .expect("Error creating upload connection");
    upload_stream
        .write_all(&contents)
        .await
        .expect("Error while uploading boffile");
    // Close stream
    upload_stream
        .shutdown()
        .await
        .expect("Error closing upload connection");
    info!("Upload complete, waiting for programming");
    // Wait ???? until we're good
    sleep(Duration::from_millis(10000)).await;
    // Check status
    match make_request(state, Fpgastatus::Request).await {
        Ok(v) => {
            if let Some(Fpgastatus::Reply { ret_code }) = v.get(0) {
                if *ret_code != RetCode::Ok {
                    panic!("FPGA Reports it's not good to go, strange");
                }
            }
        }
        Err(e) => {
            println!("{}", e);
            panic!("Requesting the FPGA status failed: we're bailing");
        }
    }
    info!("Programming successful");
}
