mod args;
mod handlers;

use clap::Parser;
use katcp::prelude::*;
use std::{collections::HashMap, error::Error, net::IpAddr, path::PathBuf};
use std::{fmt::Debug, net::SocketAddr};
use tokio::task;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
};
use tracing::debug;

use args::*;
use handlers::*;

struct State {
    unhandled_incoming_messages: UnboundedReceiver<Message>,
    // The writer
    writer: OwnedWriteHalf,
}

// async fn read_version_connect(state: &mut State) -> VersionConnect {
//     let line = state
//         .lines
//         .next_line()
//         .await
//         .expect("Error awaiting version connect line")
//         .unwrap(); // Why do I need an unwap here?
//     line.as_str()
//         .try_into()
//         .expect("Error deserializing version connect message")
// }

// async fn send_request<K: KatcpMessage + std::fmt::Debug>(
//     request: K,
//     state: &mut State,
// ) -> (Vec<K>, Option<Fpga>)
// where
//     <K as std::convert::TryFrom<katcp::prelude::Message>>::Error: std::fmt::Debug,
// {
//     // Serialize and send request
//     let request_msg = request
//         .to_message(None)
//         .expect("Could not serialize request to a KATCP message");
//     let request_name = request_msg.name();
//     state
//         .writer
//         .write_all(request_msg.to_string().as_bytes())
//         .await
//         .expect("Error writting bytes to TCP connection");

//     // Containers for collected messages
//     let mut informs_and_reply = vec![];
//     // We may recieve a number of FPGA status updates (that seemss to just randomly happen)
//     // We only care about the last one (the current FPGA status)
//     let mut status_updates: Vec<Fpga> = vec![];

//     // Keep reading lines
//     while let Some(line) = state.lines.next_line().await.unwrap() {
//         // We expect either log informs, message informs, or replys
//         let raw: Message = line.as_str().try_into().expect("Malformed KATCP message");
//         let new_msg_type = raw.name();
//         if new_msg_type != request_name && new_msg_type != "log" && new_msg_type != "fpga" {
//             panic!("Got unexpected KATCP message: {}", raw.name());
//         }
//         // Deal with the three cases
//         match new_msg_type.as_str() {
//             "log" => handle_log(raw.try_into().expect("Could not deserialize log message")),
//             "fpga" => {
//                 let status = raw.try_into().expect("Could not deserialize fpga stauts");
//                 trace!(?status);
//                 status_updates.push(status);
//             }
//             _ => match raw.kind() {
//                 MessageKind::Request => unreachable!(),
//                 MessageKind::Reply => {
//                     // Push the last reply and break
//                     let reply = raw.try_into().expect("Could not deserialize KATCP reply");
//                     trace!(?reply);
//                     informs_and_reply.push(reply);
//                     break;
//                 }
//                 MessageKind::Inform => {
//                     let inform = raw.try_into().expect("Could not deserialize KATCP infrom");
//                     trace!(?inform);
//                     informs_and_reply.push(inform);
//                 }
//             },
//         }
//     }
//     (informs_and_reply, status_updates.last().cloned())
// }

// /// Returns a vector of bof-files present on the device
// async fn get_bofs(state: &mut State) -> Vec<String> {
//     // Send a listbof message and collect what comes back (no FPGA status updates)
//     let (replies, _) = send_request(Listbof::Request, state).await;
//     if let Listbof::Reply(IntReply::Ok { num }) = replies.last().unwrap() {
//         assert_eq!(
//             *num,
//             (replies.len() as u32) - 1,
//             "We didn't recieve as many files as we were told to expect"
//         );
//         replies
//             .iter()
//             .take(*num as usize)
//             .map(|inform| {
//                 if let Listbof::Inform { filename } = inform {
//                     filename.clone()
//                 } else {
//                     unreachable!()
//                 }
//             })
//             .collect()
//     } else {
//         panic!("Last message from request wasn't a reply");
//     }
// }

// async fn program_bof(filename: String, force: bool, state: &mut State) {
//     // First get the list of bofs
//     let bofs = get_bofs(state).await;
//     if bofs.iter().any(|e| *e == filename) && !force {
//         // Upload the file that's already on board
//         let (reply, status) = send_request(
//             Progdev::Request {
//                 filename: filename.clone(),
//             },
//             state,
//         )
//         .await;
//     } else {
//         // Upload the file directly and then try to program
//     }
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Grab the command line arguments
    let args = Args::parse();
    // install global collector configured based on RUST_LOG env var.
    tracing_subscriber::fmt::init();
    debug!("Logging started");
    // Create the channels
    let (tx, rx) = unbounded_channel::<Message>();
    // Connect to the SNAP katcp server
    let (reader, writer) = TcpStream::connect(SocketAddr::new(args.address, args.port))
        .await?
        .into_split();
    // Startup dispatcher
    dispatch_katcp_messages(tx, reader, make_dispatchers()).await;
    Ok(())

    // // Setup the program state
    // let mut state = State {
    //     lines: BufReader::new(reader).lines(),
    //     writer,
    // };

    // // Read the first three informs that give us system information
    // let device_lib = read_version_connect(&mut state).await;
    // let device_protocol = read_version_connect(&mut state).await;
    // let device_kernel = read_version_connect(&mut state).await;
    // debug!(?device_lib);
    // debug!(?device_protocol);
    // debug!(?device_kernel);
    // // Perform the action
    // match args.command {
    //     Command::Load { path, force } => {
    //         program_bof(
    //             path.file_name().unwrap().to_str().unwrap().to_owned(),
    //             force,
    //             &mut state,
    //         )
    //         .await;
    //         Ok(())
    //     }
    // }
}
