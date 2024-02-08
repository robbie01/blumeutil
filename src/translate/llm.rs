use rusqlite::Connection;
use llama::{
    options::{ModelOptions, PredictOptions},
    LLama
};

#[derive(Clone, Debug)]
pub struct Translator {
    session: String
}

impl Translator {
    pub fn new(session: String) -> Self {
        Self { session }
    }

    pub async fn translate<'a>(&self, db: &mut Connection, script: u32) -> anyhow::Result<()> {
        Ok(())
    }
}
