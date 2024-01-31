mod google;
//mod googlefree;
//mod llm;

use std::path::PathBuf;
use anyhow::Context as _;
use clap::{Parser, ValueEnum};
use rusqlite::Connection;
use reqwest::Client;

use google::Translator as GoogleTranslator;
//use googlefree::Translator as GoogleFreeTranslator;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Provider {
    Google,
    Llm
}

#[derive(Parser)]
struct Args {
    db: PathBuf,
    provider: Provider,
    script_id: u32
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut db = Connection::open(args.db)?;

    let cli = Client::new();

    match args.provider {
        Provider::Google => {
            let mut tl = GoogleTranslator::new(db.query_row(
                "SELECT value FROM config WHERE name = 'google_api_key'",
                (),
                |row| row.get(0)
            ).context("no google_api_key configured")?);
            tl.translate(cli.clone(), &mut db, args.script_id).await?;
        },
        Provider::Llm => todo!()
    }

    Ok(())
}
