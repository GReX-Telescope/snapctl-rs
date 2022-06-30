mod adc;
mod args;
mod handlers;
mod tengbe;
mod utils;

use std::{
    error::Error,
    fmt::Debug,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use args::*;
use clap::Parser;
use handlers::*;
use katcp::{
    messages::{core::*, log::*},
    prelude::*,
};
use katcp_casper::*;
use packed_struct::prelude::*;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp::OwnedWriteHalf, TcpStream},
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
    task,
    time::{sleep, Duration},
};
use tracing::{debug, info, trace};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::{tengbe::CoreType, utils::*};

struct State {
    unhandled_incoming_messages: UnboundedReceiver<Message>,
    // The writer
    writer: OwnedWriteHalf,
    // The connection address
    address: IpAddr,
}

async fn make_request<T>(state: &mut State, request: T) -> Result<Vec<T>, String>
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

async fn ping(state: &mut State) {
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

async fn set_device_log_level(state: &mut State, lvl: Level) {
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

async fn program_bof(path: PathBuf, port: u16, state: &mut State) {
    // Upload the file directly and then try to program
    debug!("The file we want to program doesn't exist on the device (or we're forcing an upload), upload it instead");
    info!("Attempting to program: {}", path.display());
    // Get an upload port
    match make_request(state, Progremote::Request {
        port: (port as u32),
    })
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Grab the command line arguments
    let args = Args::parse();
    // install global collector configured based on RUST_LOG env var or default to info.
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(if args.verbose { "debug" } else { "info" }))
        .unwrap();
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter_layer)
        .init();
    debug!("Logging started");
    // Create the channels
    let (tx, rx) = unbounded_channel::<Message>();
    // Connect to the SNAP katcp server
    let (reader, writer) = TcpStream::connect(SocketAddr::new(args.address, args.port))
        .await?
        .into_split();
    // Startup dispatcher
    task::spawn(handle_informs(tx, reader, make_inform_dispatchers()));
    // Setup the program state
    let mut state = State {
        unhandled_incoming_messages: rx,
        writer,
        address: args.address,
    };
    // Do an initial ping to make sure we're actually connected
    ping(&mut state).await;
    info!("Connected to the SNAP");
    // Ask the device  to send us trace level logs, even if we don't use them as we'll filter them here
    set_device_log_level(&mut state, Level::Trace).await;
    // Perform the requested action
    // Perform the action
    match args.command {
        Command::Load { path, port } => program_bof(path, port, &mut state).await,
        Command::ConfigGBE { core } => config_gbe(&core, &mut state).await,
    };
    Ok(())
}

async fn wordread(register_name: &str, offset: u32, state: &mut State) -> u32 {
    match make_request(state, Wordread::Request {
        name: register_name.to_owned(),
        offset,
    })
    .await
    {
        Ok(v) => {
            if let Wordread::Reply { ret_code, word } = v.get(0).unwrap() {
                assert_eq!(*ret_code, RetCode::Ok);
                debug!("Read word successfully!");
                *word
            } else {
                panic!("Got a bad wordread response, we're bailing");
            }
        }
        Err(e) => {
            println!("{}", e);
            panic!("Reading a word errored: we're bailing");
        }
    }
}

// Test function, please ignore
async fn config_gbe(core: &str, state: &mut State) {
    // Try to read from the gbe register
    let ct = CoreType::unpack(
        &wordread(core, CoreType::address() as u32, state)
            .await
            .to_be_bytes(),
    )
    .unwrap();
    dbg!(ct);
}
