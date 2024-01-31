use rusqlite::Connection;
use clap::Parser;

#[derive(Parser)]
pub struct Args {
    name: Option<String>,
    #[arg(requires = "name")]
    value: Option<String>
}

pub fn run(db: Connection, args: Args) -> anyhow::Result<()> {
    match args {
        Args { name: None, value: None } => {
            let mut stmt = db.prepare("SELECT name, value FROM config")?;
            let mut rows = stmt.query(())?;
            while let Some(row) = rows.next()? {
                let (name, value): (String, String) = row.try_into()?;
                println!("{name} = {value}");
            }
        },
        Args { name: Some(name), value: None } => {
            let (value,): (String,) = db.query_row(
                "SELECT value FROM config WHERE name = ?",
                (&name,),
                |row| row.try_into()
            )?;
            println!("{name} = {value}");
        },
        Args { name: Some(name), value: Some(value) } => {
            db.execute("INSERT OR REPLACE INTO config VALUES(?, ?)", (name, value))?;
        },
        _ => unreachable!()
    }
    Ok(())
}
