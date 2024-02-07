use std::sync::Mutex;

use rusqlite::Connection;

pub struct Model {
    db: Mutex<Connection>
}

#[derive(Clone, Debug)]
pub struct Row {
    pub address: u32,
    pub speaker: String,
    pub original: String,
    pub control: String,
    pub current: String
}

impl TryFrom<&rusqlite::Row<'_>> for Row {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            address: row.get(0)?,
            speaker: row.get(1)?,
            original: row.get(2)?,
            control: row.get(3)?,
            current: row.get(4)?
        })
    }
}

impl Model {
    pub fn new(db: Connection) -> Self {
        Self { db: Mutex::new(db) }
    }

    pub fn translations(&self, session: &str, scriptid: u32) -> rusqlite::Result<Vec<Row>> {
        let db = self.db.lock().unwrap();

        let mut stmt = db.prepare_cached("
            SELECT lines.address, lines.speaker, lines.line, google.translation, IFNULL(current.translation, '') FROM lines
            LEFT JOIN translations AS google
                ON google.session = 'google' AND google.scriptid = lines.scriptid AND google.address = lines.address
            LEFT JOIN translations AS current
                ON current.session = ? AND current.scriptid = lines.scriptid AND current.address = lines.address
            WHERE lines.scriptid = ?
        ")?;

        let rows = stmt
            .query_map((&session, scriptid), |row| Row::try_from(row))?
            .collect::<rusqlite::Result<_>>()?;

        Ok(rows)
    }

    pub fn update_translation(&self, session: &str, scriptid: u32, address: u32, translation: &str) -> rusqlite::Result<String> {
        let db = self.db.lock().unwrap();

        db.query_row(
            "INSERT OR REPLACE INTO translations(session, scriptid, address, translation) VALUES (?, ?, ?, TRIM(?)) RETURNING translation",
            (session, scriptid, address, translation),
            |row| row.get(0)
        )
    }
}