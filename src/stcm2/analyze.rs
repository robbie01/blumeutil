use bytes::Bytes;
use rusqlite::{Connection, DropBehavior};

use super::{Args, parse, format};

pub fn analyze(mut db: Connection, args: Args) -> anyhow::Result<()> {
    let mut tx = db.transaction()?;
    tx.set_drop_behavior(DropBehavior::Commit);

    let file = tx.query_row("SELECT script FROM scripts WHERE id = ?", (args.id,), |row| Ok(Bytes::copy_from_slice(row.get_ref(0)?.as_blob()?)))?;

    let stcm2 = format::from_bytes(file)?;

    let parsed = parse::parse(stcm2.actions.into_iter().filter_map(|(addr, act)| act.op(addr.orig).ok()))?;

    let mut stmt = tx.prepare("INSERT OR IGNORE INTO lines(scriptid, address, speaker, line) VALUES (?, ?, ?, ?)")?;
    let mut n = 0;
    for d in parsed {
        if let parse::Dialogue::Line { addr, speaker, line } = d {
            stmt.execute((args.id, addr, speaker, line))?;
        } else if let parse::Dialogue::Choice { .. } = d {
            n += 1;
        }
    }
    println!("found {n} choices");

    drop(stmt);

    if args.dry_run {
        tx.rollback()?;
    } else {
        tx.commit()?;
    }

    Ok(())
}