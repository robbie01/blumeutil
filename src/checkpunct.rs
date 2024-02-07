use rusqlite::Connection;
use clap::Parser;

#[derive(Parser)]
pub struct Args {
    script_id: u32
}

const CHECKS: &[[[char; 2]; 2]] = &[
    [['（', '）'], ['(', ')']],
    [['「', '」'], ['"', '"']]
];

pub fn run(db: Connection, args: Args) -> anyhow::Result<()> {
    let mut stmt = db.prepare("
        SELECT lines.scriptid, lines.address, lines.line, translations.translation FROM lines
        LEFT JOIN translations USING (scriptid, address)
        WHERE lines.scriptid = ? AND translations.session = 'google'
    ")?;
    let mut rows = stmt.query((args.script_id,))?;

    while let Some(row) = rows.next()? {
        let (scriptid, address, line, google): (u32, u32, String, String) = row.try_into()?;

        for &[[jb, je], [eb, ee]] in CHECKS.iter() {
            if line.starts_with(jb) && line.ends_with(je) {
                if !(google.starts_with(eb) && google.ends_with(ee)) {
                    println!("{scriptid}, {address} is weird\n{line}\n{google}\n");
                }
            } else if line.contains([jb, je, eb, ee]) {
                println!("{scriptid}, {address} is weird2\n{line}\n{google}\n");
            }
            if google.contains([jb, je]) {
                println!("{scriptid}, {address} is weird3\n{line}\n{google}\n");
            }
        }
    }

    Ok(())
}