use rusqlite::Connection;
use clap::Parser;

#[derive(Parser)]
pub struct Args {}

pub fn run(mut db: Connection, _args: Args) -> anyhow::Result<()> {
    let tx = db.transaction()?;
    tx.execute_batch("
        CREATE TABLE config(name TEXT PRIMARY KEY, value ANY NOT NULL) WITHOUT ROWID, STRICT;
        CREATE TABLE scripts(id INTEGER PRIMARY KEY, script BLOB NOT NULL) STRICT;
        CREATE TABLE lines(
            scriptid INTEGER REFERENCES scripts(id),
            address INTEGER,
            speaker TEXT NOT NULL,
            line TEXT NOT NULL,
            PRIMARY KEY(scriptid, address)
        ) WITHOUT ROWID, STRICT;
        CREATE TABLE translations(
            session TEXT,
            scriptid INTEGER,
            address INTEGER,
            translation TEXT NOT NULL,
            FOREIGN KEY(scriptid, address) REFERENCES lines(scriptid, address),
            PRIMARY KEY(session, scriptid, address)
        ) WITHOUT ROWID, STRICT;
    ")?;
    tx.commit()?;
    Ok(())
}
