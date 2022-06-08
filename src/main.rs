mod args;
mod handlers;

use clap::Parser;
use katcp::{
    messages::{core::*, log::*},
    prelude::*,
};
use katcp_casper::{Listbof, Progdev};
use std::{error::Error, fmt::Debug, net::SocketAddr};
use tokio::{
    io::AsyncWriteExt,
    net::{tcp::OwnedWriteHalf, TcpStream},
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
    task,
};
use tracing::{debug, info, trace};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

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

/// Returns a vector of bof-files present on the device
async fn get_bofs(state: &mut State) -> Vec<String> {
    match make_request(state, Listbof::Request).await {
        Ok(v) => {
            // The returned vector should be all informs and the reply
            // We should check we got back the number of messages we expected
            let reply = v
                .iter()
                .find(|msg| matches!(msg, Listbof::Reply(_)))
                .expect("We didn't get a Listbof reply");
            let num_bofs = match reply {
                Listbof::Reply(IntReply::Ok { num }) => *num,
                _ => panic!("The Listbof reply contained an error code"),
            };
            assert_eq!(num_bofs, (v.len() as u32) - 1);
            // Now grab all the filenames
            v.iter()
                .filter_map(|msg| match msg {
                    Listbof::Inform { filename } => Some(filename.clone()),
                    _ => None,
                })
                .collect()
        }
        Err(e) => {
            println!("{}", e);
            panic!("Setting log level errored: we're bailing");
        }
    }
}

async fn program_bof(filename: String, force: bool, state: &mut State) {
    // First get the list of bofs
    let bofs = get_bofs(state).await;
    if bofs.iter().any(|e| *e == filename) && !force {
        // Upload the file that's already on board
        match make_request(
            state,
            Progdev::Request {
                filename: filename.clone(),
            },
        )
        .await
        {
            Ok(v) => {
                // We should have gotten one reply
                if let Some(Progdev::Reply { ret_code }) = v.get(0) {
                    if *ret_code == RetCode::Ok {
                        info!("BOF programming successful");
                    }
                }
            }
            Err(e) => {
                println!("{}", e);
                panic!("Programming the boffile failed: we're bailing");
            }
        }
    } else {
        // Upload the file directly and then try to program
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Grab the command line arguments
    let args = Args::parse();
    // install global collector configured based on RUST_LOG env var or default to info.
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
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
    };
    // Do an initial ping to make sure we're actually connected
    ping(&mut state).await;
    // Ask the device  to send us trace level logs, even if we don't use them as we'll filter them here
    set_device_log_level(&mut state, Level::Trace).await;
    // Perform the requested action
    // Perform the action
    match args.command {
        Command::Load { path, force } => {
            program_bof(
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                force,
                &mut state,
            )
            .await;
        }
    };
    Ok(())
}
