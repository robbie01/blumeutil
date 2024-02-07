use rusqlite::Connection;
use super::translate;

#[derive(Clone, Debug)]
pub struct Translator {
    session: String
}

impl Translator {
    pub fn new(session: String) -> Self {
        Self { session }
    }
}

impl Translator {
    pub fn translate(&self, db: &mut Connection, script: u32) -> anyhow::Result<()> {
        let lines = db.prepare_cached("
            SELECT address, line
            FROM lines
            WHERE scriptid = ?1
                AND (?2, ?1, address) NOT IN
                    (SELECT session, scriptid, address FROM translations)
        ")?.query_map((script, &self.session), |row| row.try_into())?.collect::<Result<Vec<(u32, String)>, _>>()?;

        for (address, line) in lines {
            println!("src: {line}");
            let tl = translate::translate(&line)?;
            println!("tl : {tl}");
            db.execute("INSERT INTO translations(session, scriptid, address, translation) VALUES(?, ?, ?, ?)", (&self.session, script, address, &tl))?;
        }

        Ok(())
    }
}
