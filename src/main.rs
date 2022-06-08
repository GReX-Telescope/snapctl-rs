use clap::{Parser, Subcommand};
use katcp::{
    messages::{
        core::VersionConnect,
        log::{Log, LogLevel},
    },
    prelude::*,
};
use katcp_casper::{Fpga, FpgaStatus, Listbof, Progdev};
use std::{error::Error, net::IpAddr, path::PathBuf};
use std::{fmt::Debug, net::SocketAddr};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};
use tracing::{debug, error, info, instrument, trace, warn};

#[derive(Subcommand, Debug)]
enum Command {
    /// Loads a bitstream (BOF) file to the SNAP
    Load {
        path: PathBuf,
        /// Overwrite file if it already exists
        #[clap(short, long)]
        force: bool,
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Args {
    #[clap(subcommand)]
    command: Command,
    /// Address of the SNAP tcpborph server
    #[clap(short, long)]
    address: IpAddr,
    /// Port of the SNAP katcp tcpborph server
    #[clap(short, long, default_value_t = 7147)]
    port: u16,
    /// Print all log messages and debug information
    #[clap(short, long)]
    verbose: bool,
}

struct State {
    // The reader (abstracted as a line reader)
    lines: Lines<BufReader<OwnedReadHalf>>,
    // The writer
    writer: OwnedWriteHalf,
}

fn handle_log(log: Log) {
    if let Log::Inform {
        level,
        name,
        message,
        ..
    } = log
    {
        match level {
            LogLevel::Error => error!("[{}] {}", name, message),
            LogLevel::Warn => warn!("[{}] {}", name, message),
            LogLevel::Info => info!("[{}] {}", name, message),
            LogLevel::Debug => debug!("[{}] {}", name, message),
            LogLevel::Trace => trace!("[{}] {}", name, message),
            _ => println!(
                "Unexpected Log: [{}] {} {}",
                level.to_argument(),
                name,
                message
            ),
        }
    }
}

async fn read_version_connect(state: &mut State) -> VersionConnect {
    let line = state
        .lines
        .next_line()
        .await
        .expect("Error awaiting version connect line")
        .unwrap(); // Why do I need an unwap here?
    line.as_str()
        .try_into()
        .expect("Error deserializing version connect message")
}

async fn send_request<K: KatcpMessage>(request: K, state: &mut State) -> (Vec<K>, Option<Fpga>)
where
    <K as std::convert::TryFrom<katcp::prelude::Message>>::Error: std::fmt::Debug,
{
    // Serialize and send request
    let request_msg = request
        .to_message(None)
        .expect("Could not serialize request to a KATCP message");
    let request_name = request_msg.name();
    state
        .writer
        .write_all(request_msg.to_string().as_bytes())
        .await
        .expect("Error writting bytes to TCP connection");

    // Containers for collected messages
    let mut informs_and_reply = vec![];
    // We may recieve a number of FPGA status updates (that seemss to just randomly happen)
    // We only care about the last one (the current FPGA status)
    let mut status_updates: Vec<Fpga> = vec![];

    // Keep reading lines
    while let Some(line) = state.lines.next_line().await.unwrap() {
        // We expect either log informs, message informs, or replys
        let raw: Message = line.as_str().try_into().expect("Malformed KATCP message");
        let new_msg_type = raw.name();
        if new_msg_type != request_name && new_msg_type != "log" && new_msg_type != "fpga" {
            panic!("Got unexpected KATCP message: {}", raw.name());
        }
        // Deal with the three cases
        match new_msg_type.as_str() {
            "log" => handle_log(raw.try_into().expect("Could not deserialize log message")),
            "fpga" => {
                status_updates.push(raw.try_into().expect("Could not deserialize fpga stauts"))
            }
            _ => match raw.kind() {
                MessageKind::Request => unreachable!(),
                MessageKind::Reply => {
                    // Push the last reply and break
                    informs_and_reply
                        .push(raw.try_into().expect("Could not deserialize KATCP reply"));
                    break;
                }
                MessageKind::Inform => informs_and_reply
                    .push(raw.try_into().expect("Could not deserialize KATCP inform")),
            },
        }
    }
    (informs_and_reply, status_updates.last().cloned())
}

/// Returns a vector of bof-files present on the device
async fn get_bofs(state: &mut State) -> Vec<String> {
    // Send a listbof message and collect what comes back (no FPGA status updates)
    let (replies, _) = send_request(Listbof::Request, state).await;
    if let Listbof::Reply(IntReply::Ok { num }) = replies.last().unwrap() {
        assert_eq!(
            *num,
            (replies.len() as u32) - 1,
            "We didn't recieve as many files as we were told to expect"
        );
        replies
            .iter()
            .take(*num as usize)
            .map(|inform| {
                if let Listbof::Inform { filename } = inform {
                    filename.clone()
                } else {
                    unreachable!()
                }
            })
            .collect()
    } else {
        panic!("Last message from request wasn't a reply");
    }
}

async fn program_bof(filename: String, force: bool, state: &mut State) {
    // First get the list of bofs
    let bofs = get_bofs(state).await;
    // Force an upload if we've set --force
    // Query for an upload port
    // Perform the upload
    // Try to program
    let (reply, status) = send_request(
        Progdev::Request {
            filename: filename.clone(),
        },
        state,
    )
    .await;
    if let Some(Fpga::Inform {
        status: FpgaStatus::Ready,
    }) = status
    {
        info!("SNAP programmed and mapped with {}", &filename);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt::init();
    info!("Logging started!");
    // Grab the command line arguments
    let args = Args::parse();
    // Connect to the SNAP katcp server
    let (reader, writer) = TcpStream::connect(SocketAddr::new(args.address, args.port))
        .await?
        .into_split();
    // Setup the program state
    let mut state = State {
        lines: BufReader::new(reader).lines(),
        writer,
    };
    // Read the first three informs that give us system information
    let _ = read_version_connect(&mut state).await;
    let _ = read_version_connect(&mut state).await;
    let _ = read_version_connect(&mut state).await;
    // Perform the action
    match args.command {
        Command::Load { path, force } => {
            // let bofs = get_bofs(&mut state).await;
            // println!("{:#?}", bofs);
            program_bof(
                path.file_name()
                    .expect("bof file does not exist")
                    .to_str()
                    .unwrap()
                    .to_owned(),
                force,
                &mut state,
            )
            .await;
            Ok(())
        }
    }
}
