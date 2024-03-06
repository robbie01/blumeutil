mod format;
mod parse;
mod analyze;
mod patch;
mod actions;

use rusqlite::Connection;
use clap::{Parser, ValueEnum};

#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum)]
enum Mode {
    Analyze,
    Patch
}

#[derive(Parser)]
pub struct Args {
    mode: Mode,
    #[arg(help = "id of scripts to analyze")]
    id: u32,
    #[arg(from_global)]
    dry_run: bool
}

pub fn run(db: Connection, args: Args) -> anyhow::Result<()> {
    match args.mode {
        Mode::Analyze => analyze::analyze(db, args),
        Mode::Patch => patch::patch(db, args)
    }
}
