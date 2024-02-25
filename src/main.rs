#![recursion_limit = "512"] // for html
#![allow(clippy::write_with_newline)] // for consistency
#![allow(clippy::manual_non_exhaustive)] // no, clippy, non_exhaustive doesnt apply to modules

mod config;
mod init;
mod deuni;
mod stcm2;
mod translate;
mod web;
mod cleanup;
mod checkpunct;

use std::path::PathBuf;
use rusqlite::{Connection, OpenFlags};
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    #[arg(short, help = "Path to the blume working database file")]
    file: PathBuf,
    #[arg(short = 'n', global = true, help = "don't actually write to the database")]
    dry_run: bool,
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
    Translate(translate::Args),
    Web(web::Args),
    Cleanup(cleanup::Args),
    Checkpunct(checkpunct::Args)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    
    tracing_subscriber::fmt::init();

    let db = match args.command {
        Init(_) => Connection::open(args.file)?,
        // open without creating if not init
        _ => Connection::open_with_flags(
            args.file,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX
        )?
    };
    db.pragma_update(None, "foreign_keys", true)?;

    use Command::*;
    match args.command {
        Config(margs) => config::run(db, margs),
        Init(margs) => init::run(db, margs),
        DeUni(margs) => deuni::run(db, margs),
        Stcm2(margs) => stcm2::run(db, margs),
        Translate(margs) => translate::run(db, margs).await,
        Web(margs) => web::run(db, margs).await,
        Cleanup(margs) => cleanup::run(db, margs),
        Checkpunct(margs) => checkpunct::run(db, margs)
    }
}
