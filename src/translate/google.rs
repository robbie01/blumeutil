use std::iter;
use anyhow::{bail, Context as _};
use reqwest::Client;
use rusqlite::{Connection, DropBehavior};
use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub struct Translator(String);

impl Translator {
    pub fn new(api_key: String) -> Self {
        Self(api_key)
    }
}

impl Translator {
    pub async fn translate(&mut self, client: Client, db: &mut Connection, script: u32) -> anyhow::Result<()> {
        let lines = db.prepare_cached("
            SELECT address, line
            FROM lines
            WHERE scriptid = ?1
                AND ('google', ?1, address) NOT IN
                    (SELECT session, scriptid, address FROM translations)
        ")?.query_map((script,), |row| row.try_into())?.collect::<Result<Vec<(u32, String)>, _>>()?;

        for chunk in lines.chunks(128) {
            println!("translating {} lines", chunk.len());

            let mut tx = db.transaction()?;
            tx.set_drop_behavior(DropBehavior::Commit);

            let res = client
                .post("https://translation.googleapis.com/language/translate/v2")
                .header("X-Goog-Api-Key", &self.0)
                .json(&json!({
                    "q": chunk.iter().map(|(_, l)| l.as_ref()).collect::<Vec<&str>>(),
                    "target": "en",
                    "format": "text",
                    "source": "ja"
                }))
                .send().await?;

            if !res.status().is_success() {
                let d = format!("{res:?}");
                bail!("bad response: {d}\n{}", res.text().await?);
            }

            let res = res.json::<Value>().await?;

            let tls = res
                .pointer("/data/translations").context("no translations")?
                .as_array().context("translations is not array")?;

            {
                let mut stmt = tx.prepare_cached("INSERT INTO lines VALUES('google', ?, ?, ?)")?;

                for ((addr, _orig), tl) in iter::zip(chunk, tls) {
                    // make insertions resilient; try to salvage as much data as possible
                    match tl.pointer("/translatedText").and_then(Value::as_str) {
                        None => eprintln!("warning: script {script} addr {addr} has an invalid or missing translatedText"),
                        Some(line) => {
                            if let Err(e) = stmt.execute((script, addr, line)) {
                                eprintln!("warning: script {script} addr {addr} failed to save");
                                eprintln!("line: {line}");
                                eprintln!("error: {e}");
                            }
                        }
                    }
                }
            }

            tx.commit()?;
        }

        Ok(())
    }
}
