mod adc;
mod api;
mod args;
mod handlers;
mod tengbe;
mod utils;

use std::{error::Error, net::SocketAddr};

use args::*;
use clap::Parser;
use handlers::*;
use katcp::{messages::log::*, prelude::*};
use tokio::{net::TcpStream, sync::mpsc::unbounded_channel, task};
use tracing::{debug, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::{api::*, utils::*};

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
    set_device_log_level(&mut state, Level::Info).await;
    // Perform the requested action
    // Perform the action
    match args.command {
        Command::Upload { path, port } => upload(path, port, &mut state).await,
        Command::ConfigGBE { core } => config_gbe(&core, &mut state).await,
    };
    Ok(())
}
