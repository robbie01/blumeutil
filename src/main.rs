#![cfg_attr(feature = "web", recursion_limit = "512")] // for html

mod config;
mod init;
mod uni;
mod stcm2;
#[cfg(feature = "translate")]
mod translate;
#[cfg(feature = "web")]
mod web;
mod cleanup;
mod checkpunct;
mod script;
// mod iso;

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
    Uni(uni::Args),
    Stcm2(stcm2::Args),
    #[cfg(feature = "translate")]
    Translate(translate::Args),
    #[cfg(feature = "web")]
    Web(web::Args),
    Cleanup(cleanup::Args),
    Checkpunct(checkpunct::Args),
    Script(script::Args)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    
    #[cfg(feature = "web")]
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
        Uni(margs) => uni::run(db, margs),
        Stcm2(margs) => stcm2::run(db, margs),
        #[cfg(feature = "translate")]
        Translate(margs) => translate::run(db, margs).await,
        #[cfg(feature = "web")]
        Web(margs) => web::run(db, margs).await,
        Cleanup(margs) => cleanup::run(db, margs),
        Checkpunct(margs) => checkpunct::run(db, margs),
        Script(margs) => script::run(db, margs)
    }
}
