mod similar;

pub use reqwest;
use rusqlite::{Connection, DropBehavior};

use std::{borrow::Cow, fmt::Write as _};
use reqwest::Client;
use serde_json::{json, Value};
use anyhow::anyhow;

#[derive(Clone, Debug)]
struct Seen {
    speaker: String,
    line: String,
    tled: String
}

fn build_prompt(seen: &[Seen], speaker: &str, line: &str) -> anyhow::Result<String> {
    let mut prompt =
r#"<s>[INST] You are a translator working on the visual novel Edel Blume, a Japanese otome game. You will receive scripts in CSV format with the columns "Speaker" and "Dialogue" and translate them faithfully into English with quality prose and grammar. [/INST]
Understood. I'll make sure that every line is translated accurately.</s>
[INST] Speaker,Dialogue
"#.to_owned();

    for Seen { speaker, line, ..} in seen.iter() {
        writeln!(prompt, "{speaker},{line}")?;
    }
    writeln!(prompt, "{speaker},{line} [/INST]")?;

    writeln!(prompt, "Speaker,Dialogue")?;
    for Seen { speaker, tled, .. } in seen.iter() {
        writeln!(prompt, "{speaker},{tled}")?;
    }
    
    write!(prompt, "{speaker},")?;

    Ok(prompt)
}

fn improve_correlation(line: &str) -> String {
    const REPLACEMENTS: &[(&str, &str)] = &[
        ("……", "..."),
        ("「", "\""),
        ("」", "\""),
        ("、", ","),
        ("。", "."),
        ("？", "?"),
        ("！", "!"),
        ("（", "("),
        ("）", ")")
    ];

    let mut line = Cow::Borrowed(line);

    for &(orig, subst) in REPLACEMENTS.iter() {
       line = line.replace(orig, subst).into();
    }

    line.into_owned() 
}

#[derive(Clone, Debug)]
pub struct Translator {
    session: String
}

impl Translator {
    pub fn new(session: String) -> Self {
        Self { session }
    }

    pub async fn translate<'a>(&self, client: Client, db: &mut Connection, script: u32) -> anyhow::Result<()> {
        let mut tx = db.transaction()?;
        tx.set_drop_behavior(DropBehavior::Commit);

        let mut stmt = tx.prepare("
            SELECT lines.address, lines.speaker, lines.line, google.translation, IFNULL(current.translation, '') FROM lines
            LEFT JOIN translations AS google
                ON google.session = 'google' AND google.scriptid = lines.scriptid AND google.address = lines.address
            LEFT JOIN translations AS current
                ON current.session = ? AND current.scriptid = lines.scriptid AND current.address = lines.address
            WHERE lines.scriptid = ?
        ")?;

        let mut rows = stmt.query_map(
            (&self.session, script),
            |row| <(u32, String, String, String, String)>::try_from(row)
        )?;

        let mut seen = Vec::new();

        while let Some((address, speaker, line, control, tled)) = rows.next().transpose()? {
            if !tled.is_empty() {
                seen.push(Seen { speaker, line, tled });
                continue;
            }

            eprintln!("{speaker}: {line}");

            let ctl_score = similar::similar(&line, vec![&control])?[0].max(0.5);

            eprintln!("control score: {ctl_score}");

            let mut bans = Vec::new();
            if !line.contains(['（', '）']) {
                bans.extend([325, 28731, 4734, 17169, 20263]);
            }
            if !line.contains(['「', '」', '『', '』']) {
                bans.extend([345, 4734, 17169, 20263]);
            }

            let query = json!({
                "prompt": build_prompt(&seen, &speaker, &line)?,
                "max_tokens": 50,
                "stop": "\n",
                //"top_p": 0.5,
                "custom_token_bans": bans.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
            });

            let mut n = 0;
            let tled = loop {
                let r = client.post("http://127.0.0.1:5000/v1/completions").json(&query).send().await?.json::<Value>().await?;
                let tled = r.pointer("/choices/0/text").and_then(|s| s.as_str()).ok_or(anyhow!("couldn't get text! {r}"))?.trim();

                n += 1;
                eprintln!("attempt {n}: {tled}");

                let [mscore, cscore] = similar::similar(
                    tled,
                    vec![&improve_correlation(&line), &control]
                )?[..] else { unreachable!() };
                eprintln!("multi   score: {mscore}");
                eprintln!("control score: {cscore}");

                if mscore >= ctl_score && cscore >= 0.7 {
                    break tled.to_owned();
                }
            };

            tx.execute("INSERT OR REPLACE INTO translations(session, scriptid, address, translation) VALUES(?, ?, ?, ?)", (&self.session, script, address, &tled))?;
            seen.push(Seen { speaker, line, tled });

            eprintln!();
        }

        Ok(())
    }
}
