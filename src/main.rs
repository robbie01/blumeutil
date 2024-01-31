mod config;
mod init;
mod deuni;
mod stcm2;
mod translate;

use std::path::PathBuf;
use rusqlite::{Connection, OpenFlags};
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
    Stcm2(stcm2::Args),
    Translate(translate::Args)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let db = match args.command {
        Init(_) => Connection::open(args.file)?,
        _ => Connection::open_with_flags(
            args.file,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX
        )?
    };

    use Command::*;
    match args.command {
        Config(margs) => config::run(db, margs),
        Init(margs) => init::run(db, margs),
        DeUni(margs) => deuni::run(db, margs),
        Stcm2(margs) => stcm2::run(db, margs),
        Translate(margs) => translate::run(db, margs).await
    }
}
