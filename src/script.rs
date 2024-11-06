use std::{fs::File, io::{self, SeekFrom, Seek as _}, path::PathBuf};

use anyhow::ensure;
use clap::{Parser, Subcommand};
use rusqlite::{blob::ZeroBlob, Connection, DatabaseName};

#[derive(Parser)]
pub struct Args {
    #[arg(short = 'p')]
    patched: bool,
    #[command(subcommand)]
    mode: Mode
}

#[derive(Clone, Subcommand)]
enum Mode {
    Read { script: u32 },
    Insert { input: PathBuf, script: u32 }
}

pub fn run(mut db: Connection, args: Args) -> anyhow::Result<()> {
    let table = if args.patched { "patchedscripts" } else { "scripts" };
    match args.mode {
        Mode::Read { script } => {
            io::copy(
                &mut db.blob_open(
                    DatabaseName::Main,
                    table,
                    "script",
                    script.into(),
                    true
                )?,
                &mut io::stdout()
            )?;
        },
        Mode::Insert { input, script } => {
            let mut f = File::open(input)?;
            let len = f.seek(SeekFrom::End(0))?;
            f.seek(SeekFrom::Start(0))?;
            let tx = db.transaction()?;
            tx.execute(
                &format!(
                    "INSERT OR REPLACE INTO {} VALUES (?, ?)",
                    table
                ),
                (script, ZeroBlob(len.try_into()?))
            )?;
            let written = io::copy(
                &mut f,
                &mut tx.blob_open(
                    DatabaseName::Main,
                    table,
                    "script",
                    script.into(),
                    false
                )?
            )?;
            ensure!(written == len);
            tx.commit()?;
        }
    }

    Ok(())
}