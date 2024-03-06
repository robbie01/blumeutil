mod analyze;
mod build;

use std::path::PathBuf;
use clap::{Parser, ValueEnum};
use rusqlite::Connection;

const UNI2_MAGIC: &[u8] = b"UNI2\0\0\x01\0";
const SECTOR_SIZE: u64 = 0x800;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Mode {
    Analyze,
    Build
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    id: u32,
    start_sect: u64,
    size_sect: u64,
    size: u64
}

#[derive(Parser)]
pub struct Args {
    mode: Mode,
    #[arg(help = "Path to the uni file")]
    uni: PathBuf
}

pub fn run(db: Connection, args: Args) -> anyhow::Result<()> {
    match args.mode {
        Mode::Analyze => analyze::analyze(db, args),
        Mode::Build => build::build(db, args)
    }
}
