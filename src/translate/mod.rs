mod google;
mod llm;

use anyhow::Context as _;
use clap::{Parser, ValueEnum};
use rusqlite::Connection;
use reqwest::Client;

//use googlefree::Translator as GoogleFreeTranslator;
use google::Translator as GoogleTranslator;
use llm::Translator as LlmTramslator;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Provider {
    Google,
    Llm
}

#[derive(Parser)]
pub struct Args {
    provider: Provider,
    script_id: u32
}

pub async fn run(mut db: Connection, args: Args) -> anyhow::Result<()> {
    match args.provider {
        Provider::Google => {
            let cli = Client::new();
            let tl = GoogleTranslator::new(db.query_row(
                "SELECT value FROM config WHERE name = 'google_api_key'",
                (),
                |row| row.get(0)
            ).context("no google_api_key configured")?);
            tl.translate(cli, &mut db, args.script_id).await?;
        },
        Provider::Llm => {
            let tl = LlmTramslator::new("llm-20240201".to_owned());
            tl.translate(&mut db, args.script_id).await?;
        }
    }

    Ok(())
}
