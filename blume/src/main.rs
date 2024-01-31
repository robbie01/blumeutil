mod config;
mod init;
mod deuni;
mod stcm2;

use std::path::PathBuf;
use rusqlite::Connection;
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    #[arg(short, help = "Path to the blume working database file")]
    file: PathBuf,
    #[command(subcommand)]
    command: Command
}

#[derive(Subcommand)]
enum Command {
    Config(config::Args),
    Init(init::Args),
    #[command(name = "deuni")]
    DeUni(deuni::Args),
    Stcm2(stcm2::Args)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let db = Connection::open(args.file)?;

    use Command::*;
    match args.command {
        Config(margs) => config::run(db, margs),
        Init(margs) => init::run(db, margs),
        DeUni(margs) => deuni::run(db, margs),
        Stcm2(margs) => stcm2::run(db, margs)
    }
}
