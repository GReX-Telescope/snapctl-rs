use clap::{Parser, Subcommand};
use std::{net::IpAddr, path::PathBuf};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Loads a bitstream (BOF) file to the SNAP
    Load {
        path: PathBuf,
        /// Overwrite file if it already exists
        #[clap(short, long)]
        force: bool,
        /// The port to upload data through (separate from the katcp port)
        #[clap(long, default_value_t = 3000)]
        port: u16,
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
pub(crate) struct Args {
    /// Address of the SNAP tcpborph server
    pub(crate) address: IpAddr,
    #[clap(subcommand)]
    pub(crate) command: Command,
    /// Port of the SNAP katcp tcpborph server
    #[clap(short, long, default_value_t = 7147)]
    pub(crate) port: u16,
    /// Print all log messages and debug information
    #[clap(short, long)]
    pub(crate) verbose: bool,
}
