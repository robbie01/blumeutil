use std::{io, path::PathBuf};

use clap::Parser;
use rusqlite::{Connection, DatabaseName, OpenFlags};

#[derive(Parser)]
struct Args {
    file: PathBuf,
    script: u32
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let db = Connection::open_with_flags(
        args.file,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX
    )?;

    io::copy(
        &mut db.blob_open(DatabaseName::Main, "scripts", "script", args.script.into(), true)?,
        &mut io::stdout()
    )?;

    Ok(())
}