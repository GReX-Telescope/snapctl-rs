use std::collections::HashMap;

use katcp::{
    messages::{core::*, log::*},
    prelude::*,
};
use katcp_casper::*;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::tcp::OwnedReadHalf,
    sync::mpsc::UnboundedSender,
};
use tracing::{debug, error, info, trace, warn};

fn handle_log(log_msg: Message) {
    match log_msg.try_into() {
        Ok(Log::Inform {
            level,
            name,
            message,
            ..
        }) => match level {
            Level::Error => error!(%name, %message),
            Level::Warn => warn!(%name, %message),
            Level::Info => info!(%name, %message),
            Level::Debug => debug!(%name, %message),
            Level::Trace => trace!(%name, %message),
            _ => println!(
                "Unexpected Log: [{}] {} {}",
                level.to_argument(),
                name,
                message
            ),
        },
        Err(e) => error!(?e, "Couldn't deserialize `log`"),
    };
}

fn handle_fpga(fpga_msg: Message) {
    match fpga_msg.try_into() {
        Ok(Fpga::Inform { status }) => match status {
            FpgaStatus::Loaded => info!("FPGA Loaded"),
            FpgaStatus::Ready => info!("FPGA Ready"),
            FpgaStatus::Down => info!("FPGA Down"),
        },
        Err(e) => error!(?e, "Couldn't deserialize `fpga`"),
    };
}

fn handle_version_connect(vc_msg: Message) {
    match vc_msg.try_into() {
        Ok(vc @ VersionConnect::Inform(_)) => debug!(?vc),
        Err(e) => error!(?e, "Couldn't deserialize `version-connect`"),
    }
}

pub(crate) fn make_inform_dispatchers() -> Dispatchers {
    let mut dispatchers: Dispatchers = HashMap::new();
    dispatchers.insert("log".to_owned(), Box::new(handle_log));
    dispatchers.insert("fpga".to_owned(), Box::new(handle_fpga));
    dispatchers.insert(
        "version-connect".to_owned(),
        Box::new(handle_version_connect),
    );
    dispatchers
}

pub(crate) async fn handle_informs(
    sender: UnboundedSender<Message>,
    reader: OwnedReadHalf,
    mut dispatchers: Dispatchers,
) {
    // Read from the TCP connection, create messages, and send to the channel
    // This is only reading katcp messages from TCP
    let mut lines = BufReader::new(reader).lines();
    loop {
        // Grab message (or an empty line)
        let incoming_line = lines
            .next_line()
            .await
            .expect("Error awaiting for an incoming line. This was probably a socket error?");
        if let Some(line) = incoming_line {
            if line.is_empty() {
                continue;
            }
            let msg: Message = line
                .as_str()
                .try_into()
                .expect("Fatal error while trying to deserialize incoming KATCP message");
            // Only handle (async) informs, otherwise just push to the channel
            if msg.kind() != MessageKind::Inform {
                sender.send(msg).expect(
                    "We tried to write to the message channel, but the channel has been closed",
                );
                continue;
            }
            // If we have a dispatcher for this type, do the thing
            if let Some(dispatch_fn) = dispatchers.get_mut(&msg.name()) {
                dispatch_fn(msg);
            } else {
                // Put the unprocessed message on the channel
                sender.send(msg).expect(
                    "We tried to write to the message channel, but the channel has been closed",
                );
            }
        } else {
            warn!("Socket was closed, but not in a bad way");
            break;
        }
    }
}

pub(crate) type MessageName = String;
pub(crate) type DispatchFn = Box<dyn FnMut(Message) + Send>;
pub(crate) type Dispatchers = HashMap<MessageName, DispatchFn>;
