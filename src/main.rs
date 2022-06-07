use clap::{Parser, Subcommand};
use katcp::{
    messages::{core::VersionConnect, log::Log},
    prelude::*,
};
use katcp_casper::Listbof;
use std::fmt::Debug;
use std::{error::Error, net::IpAddr, path::PathBuf};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

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
    port: u32,
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

async fn send_request<K: KatcpMessage>(request: K, state: &mut State) -> (Vec<K>, Vec<Log>)
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
    let mut logs = vec![];
    let mut informs_and_reply = vec![];

    // Keep reading lines
    while let Some(line) = state.lines.next_line().await.unwrap() {
        // We expect either log informs, message informs, or replys
        let raw: Message = line.as_str().try_into().expect("Malformed KATCP message");
        let new_msg_type = raw.name();
        if new_msg_type != request_name && new_msg_type != "log" {
            panic!("Got unexpected KATCP message: {}", raw.name());
        }
        // Deal with the two cases
        match new_msg_type.as_str() {
            "log" => logs.push(raw.try_into().expect("Could not deserialize log message")),
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
    (informs_and_reply, logs)
}

async fn get_bofs(state: &mut State) -> Vec<String> {
    // Send a listbof message and collect what comes back
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Grab the command line arguments
    let args = Args::parse();
    // Connect to the SNAP katcp server
    let (reader, writer) = TcpStream::connect("10.10.1.3:7147").await?.into_split();
    // Setup the program state
    let mut state = State {
        lines: BufReader::new(reader).lines(),
        writer,
    };
    // Read the first three informs that give us system information
    let lib = read_version_connect(&mut state).await;
    println!("Got lib");
    let protocol = read_version_connect(&mut state).await;
    println!("Got protocol");
    let kernel = read_version_connect(&mut state).await;
    println!("{:#?}\n{:#?}\n{:#?}", lib, protocol, kernel);
    // Perform the action
    match args.command {
        Command::Load { .. } => {
            let bofs = get_bofs(&mut state).await;
            println!("{:#?}", bofs);
            Ok(())
        }
    }
}
