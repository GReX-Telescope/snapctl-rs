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
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
pub(crate) struct Args {
    #[clap(subcommand)]
    pub(crate) command: Command,
    /// Address of the SNAP tcpborph server
    #[clap(short, long)]
    pub(crate) address: IpAddr,
    /// Port of the SNAP katcp tcpborph server
    #[clap(short, long, default_value_t = 7147)]
    pub(crate) port: u16,
    /// Print all log messages and debug information
    #[clap(short, long)]
    pub(crate) verbose: bool,
}
