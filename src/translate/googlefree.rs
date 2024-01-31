use anyhow::bail;
use reqwest::Client;
use rusqlite::Connection;
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
pub struct Translator(());

impl Translator {
    pub fn new() -> Self {
        Self(())
    }
}

impl super::Translator for Translator {
    async fn translate(&mut self, client: Client, db: &Connection, script: u32, addr: u32) -> anyhow::Result<()> {
        let (speaker, line) = db.query_row("SELECT speaker, line FROM lines WHERE scriptid = ? AND address = ? AND session = 'original'", (script, addr), |row| row.try_into())?;

        let res = client
            .post("http://translate.google.com/translate_a/single?client=at&dt=t&dj=1")
            .form(&[("sl", "ja"), ("tl", "en"), ("q", &line)])
            .send().await?
            .json().await?;

        let Value::Object(mut res) = res else { bail!("bad response: {res}") };
        let Some(Value::Array(st)) = res.remove("sentences") else { bail!("bad sentences: {}", Value::Object(res)) };
        let line = st.into_iter().filter_map(|s| {
            let Value::Object(mut s) = s else { return None };
            let Some(Value::String(t)) = s.remove("trans") else { return None };
            Some(t)
        }).collect::<String>();

        db.execute("INSERT INTO lines VALUES(?, ?, 'google', ?, ?)", (script, addr, speaker, line))?;

        Ok(())
    }
}
