use std::path::PathBuf;
use rusqlite::Connection;
use clap::Parser;

#[derive(Parser)]
pub struct Args {
    key: Option<String>,
    #[arg(requires = "key")]
    value: Option<String>
}

pub fn run(file: PathBuf, args: Args) -> anyhow::Result<()> {
    let db = Connection::open(file)?;
    match args {
        Args { key: None, value: None } => {
            let mut stmt = db.prepare("SELECT key, value FROM config")?;
            let mut rows = stmt.query(())?;
            while let Some(row) = rows.next()? {
                let (key, value): (String, String) = row.try_into()?;
                println!("{key} = {value}");
            }
        },
        Args { key: Some(key), value: None } => {
            let (value,): (String,) = db.query_row(
                "SELECT value FROM config WHERE key = ?",
                (&key,),
                |row| row.try_into()
            )?;
            println!("{key} = {value}");
        },
        Args { key: Some(key), value: Some(value) } => {
            db.execute("INSERT OR REPLACE INTO config VALUES(?, ?)", (key, value))?;
        },
        _ => unreachable!()
    }
    Ok(())
}
