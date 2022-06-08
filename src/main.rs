mod args;
mod handlers;

use clap::Parser;
use katcp::{messages::core::*, prelude::*};
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
use tracing::{debug, error, trace};

use args::*;
use handlers::*;

struct State {
    unhandled_incoming_messages: UnboundedReceiver<Message>,
    // The writer
    writer: OwnedWriteHalf,
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
    trace!(?request);
    state
        .writer
        .write_all(request_msg.to_string().as_bytes())
        .await
        .expect("Error writting bytes to TCP connection");
    let mut messages = vec![];
    loop {
        match state.unhandled_incoming_messages.recv().await {
            Some(v) => {
                trace!(?v);
                match v.kind() {
                    MessageKind::Request => unreachable!(),
                    MessageKind::Inform => messages.push(
                        v.try_into()
                            .expect("Error processing incoming inform message"),
                    ),
                    MessageKind::Reply => {
                        messages.push(
                            v.try_into()
                                .expect("Error processing incoming reply message"),
                        );
                        break;
                    }
                }
            }
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
    // Setup the program state
    let mut state = State {
        unhandled_incoming_messages: rx,
        writer,
    };
    // Do an initial ping to make sure we're actually connected
    ping(&mut state).await;
    Ok(())
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
