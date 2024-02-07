use rusqlite::Connection;
use clap::Parser;

#[derive(Parser)]
pub struct Args {
    script_id: u32
}

const REPLACEMENTS: &[(&str, &str)] = &[
    ("''", "\""),
    ("”", "\""),
    ("“", "\""),
    ("``", "\""),
    ("…", "..."),
    ("（", "("),
    ("）", ")")
];

pub fn run(mut db: Connection, args: Args) -> anyhow::Result<()> {
    let tx = db.transaction()?;

    let mut stmt = tx.prepare("
        UPDATE translations
        SET translation = REPLACE(translation, ?, ?)
        WHERE session = 'google' AND scriptid = ?
    ")?;

    for &(orig, new) in REPLACEMENTS.iter() {
        stmt.execute((orig, new, args.script_id))?;
    }

    drop(stmt);
    tx.commit()?;

    Ok(())
}