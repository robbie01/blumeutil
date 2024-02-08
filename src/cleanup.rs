use rusqlite::Connection;
use clap::Parser;

#[derive(Parser)]
pub struct Args {
    script_id: u32
}

const REPLACEMENTS: &[(&str, &str)] = &[
    ("''", "\""),
    ("”", "\""),
    ("“", "\""),
    ("``", "\""),
    ("…", "..."),
    ("（", "("),
    ("）", ")")
];

const SPEAKERS: &[(&str, &str)] = &[
    ("Narrator", ""),
    ("???", "？？？"), 
    ("Ilia", "イリヤ"),
    ("Richard", "リチャード"),
    ("Daniela", "ダニエラ"),
    ("Victor", "ヴィクトル"),
    ("Mary", "#Name[1]"),
    ("Auguste", "オーギュスト"),
    ("Jacob", "ヤコブ"),
    ("Auguste's Voice", "オーギュストの声"),
    ("Virginia", "バージニア"),
    ("Klaus", "クラウス"),
    ("Stefan", "ステファン"),
    ("Relm", "レルム"),
    ("Gerald", "ジェラルド"),
    ("Conrad", "コンラッド")
];

pub fn run(mut db: Connection, args: Args) -> anyhow::Result<()> {
    let tx = db.transaction()?;

    {
        let mut stmt = tx.prepare("
            UPDATE translations
            SET translation = REPLACE(translation, ?, ?)
            WHERE session = 'google' AND scriptid = ?
        ")?;

        for &(orig, new) in REPLACEMENTS.iter() {
            stmt.execute((orig, new, args.script_id))?;
        }
    }

    {
        let mut stmt = tx.prepare("
            UPDATE lines
            SET speaker = ?
            WHERE speaker = ?
        ")?;

        for &(orig, new) in SPEAKERS.iter() {
            stmt.execute((new, orig))?;
        }
    }

    tx.commit()?;

    Ok(())
}