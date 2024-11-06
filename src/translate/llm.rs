#![allow(clippy::write_with_newline)]

mod characters;

use std::{collections::HashSet, fmt::{Display, Write as _}};

use anyhow::Context;
use reqwest::Client;
use serde_json::json;
use rusqlite::{Connection, DropBehavior};

use characters::{decode_jp_speaker, Character, EnSpeaker};

#[derive(Debug)]
pub struct Translator {
    session: String
}

#[derive(Clone, Debug)]
struct Seen {
    speaker: Option<(String, String)>,
    jpline: String,
    enline: String
}

impl Display for Seen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<<JAPANESE>>\n")?;
        if let Some((ref jpspeaker, _)) = self.speaker {
            write!(f, "[{jpspeaker}]: ")?;
        }
        write!(f, "{}\n<<ENGLISH>>\n", &self.jpline)?;
        if let Some((_, ref enspeaker)) = self.speaker {
            write!(f, "[{enspeaker}]: ")?;
        }
        write!(f, "{}<|end_of_text|>", &self.enline)?;

        Ok(())
    }
}

fn build_header(seen: &[Seen], next_speaker: Option<&str>, next_line: &str) -> anyhow::Result<String> {
    let mut cs = seen.iter()
        .filter_map(|s| s.speaker.as_ref())
        .map(|(j, _)| j.as_str())
        .chain(next_speaker)
        .filter_map(|j| match decode_jp_speaker(j) {
            Ok(EnSpeaker::Str(_)) => None,
            Ok(EnSpeaker::Character(c)) => Some(Ok(c)),
            Err(e) => Some(Err(e))
        })
        .collect::<anyhow::Result<HashSet<&Character>>>()?;

    for c in characters::CHARACTERS.iter() {
        if cs.contains(c) { continue }
        if next_line.contains(c.jpspeaker) {
            cs.insert(c);
            continue
        }
        for (a, _) in c.aliases.iter() {
            if next_line.contains(a) {
                cs.insert(c);
                continue
            }
        }
    }

    let mut header = "<|begin_of_text|><<METADATA>>\n".to_owned();
    for c in cs {
        write!(header, "[character] {c}\n")?;
    }
    //write!(header, "[element] Name: soul (魂（ゼーレ）) | Type: Terminology\n")?;
    write!(header, "<<TRANSLATE>>\n")?;

    Ok(header)
}

fn build_prompt(seen: &[Seen], next_speaker: Option<&str>, next_line: &str) -> anyhow::Result<String> {
    let mut prompt = build_header(seen, next_speaker, next_line)?;
    for s in seen {
        write!(prompt, "{s}\n")?;
    }
    prompt.push_str("<<JAPANESE>>\n");
    if let Some(next_speaker) = next_speaker {
        write!(prompt, "[{next_speaker}]: ")?;
    }
    write!(prompt, "{next_line}\n<<ENGLISH>>\n")?;
    //if let Some(enspeaker) = next_speaker.map(decode_jp_speaker).transpose()? {
    //    write!(prompt, "[{enspeaker}]:")?;
    //}

    // force punctuation
    //match next_line.chars().next() {
    //    Some('（') => write!(prompt, "(")?,
    //    Some('「') => write!(prompt, "\"")?,
    //    _ => ()
    //}

    Ok(prompt)
}

#[derive(Clone, Debug)]
struct MaxTokensReachedError(String);

impl Display for MaxTokensReachedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("maximum number of tokens reached: ")?;
        f.write_str(&self.0)
    }
}

impl std::error::Error for MaxTokensReachedError {}

async fn tokenize(client: &Client, content: &str) -> anyhow::Result<Vec<u32>> {
    client
        .post("http://127.0.0.1:8080/tokenize")
        .json(&json!({ "content": content }))
        .send().await?.error_for_status()?
        .json::<serde_json::Value>().await?
        .pointer("/tokens").context("no tokens")?
        .as_array().context("tokens is not array")?
        .iter().map(|n| Ok(n.as_u64().context("not number")?.try_into()?)).collect()
}

async fn get_completion(client: &Client, prompt: &[u32], speaker: &str) -> anyhow::Result<String> {
    let resp = client
        .post("http://127.0.0.1:8080/completion")
        .json(&if !speaker.is_empty() { json!({
             "prompt": prompt,
             "n_predict": 48,
             "grammar": format!("root ::= \"{speaker}\" [^\\n]*")
        }) } else { json!({ "prompt": prompt, "n_predict": 48 }) })
        .send().await?.error_for_status()?
        .json::<serde_json::Value>().await?;
    
    let content = resp
        .pointer("/content").context("no content")?
        .as_str().context("content is not string")?.to_owned();

    let truncated = resp
        .pointer("/truncated")
        .and_then(|t| t.as_bool())
        .unwrap_or(false);

    if truncated {
        Err(MaxTokensReachedError(content).into())
    } else {
        Ok(content)
    }
}

impl Translator {
    pub fn new(session: String) -> anyhow::Result<Self> {
        Ok(Self { session })
    }

    pub async fn translate(&self, cli: Client, db: &mut Connection, script: u32) -> anyhow::Result<()> {
        let mut seen = Vec::new();

        let mut tx = db.transaction()?;
        tx.set_drop_behavior(DropBehavior::Commit);
        let mut stmt = tx.prepare_cached("
            SELECT lines.address, lines.speaker, lines.line, translations.translation
            FROM lines LEFT JOIN translations ON
                lines.scriptid = translations.scriptid AND
                lines.address = translations.address AND
                translations.session = ?
            WHERE lines.scriptid = ?
            ORDER BY lines.address
        ")?;

        let mut rows = stmt.query((&self.session, script))?;
        while let Some(row) = rows.next()? {
            let (address, mut speaker, mut line, translation) = <(u32, String, String, Option<String>)>::try_from(row)?;
            if speaker == "#Name[1]" {
                "メアリ".clone_into(&mut speaker);
            }
            line = line.replace("#Name[1]", "メアリ");

            if let Some(translation) = translation {
                seen.push(Seen {
                    speaker: if speaker.is_empty() { None } else { Some({
                        let decoded = decode_jp_speaker(&speaker)?.to_string();
                        (speaker, decoded)
                    }) },
                    jpline: line,
                    enline: translation
                });
                continue;
            }

            eprintln!("address = {address}");
            let speaker_prefix = if speaker.is_empty() { speaker.clone() } else {
                format!("[{}]: ", decode_jp_speaker(&speaker)?)
            };

            let translation = loop {
                let prompt = loop {
                    let prompt = build_prompt(&seen, (!speaker.is_empty()).then_some(&speaker), &line)?;
                    let tokens = tokenize(&cli, &prompt).await?;
                    if tokens.len() > 8192-48 {
                        seen.remove(0);
                        continue;
                    }
                    break tokens
                };


                match get_completion(&cli, &prompt, &speaker_prefix).await {
                    Ok(tl) => break tl.strip_prefix(&speaker_prefix).unwrap().trim().to_owned(),
                    Err(e) => return Err(e)
                    /*Err(e) => match e.downcast::<MaxTokensReachedError>() {
                        Ok(e) => {
                            eprintln!("{e}");
                            seen.remove(0);
                            continue
                        },
                        Err(e) => return Err(e)
                    }*/
                }
            };
            
            eprintln!("{speaker_prefix}{translation}\n");
            tx.execute("
                INSERT INTO translations(session, scriptid, address, translation)
                VALUES (?, ?, ?, ?)
            ", (&self.session, script, address, &translation))?;
            seen.push(Seen {
                speaker: if speaker.is_empty() { None } else { Some({
                    let decoded = decode_jp_speaker(&speaker)?.to_string();
                    (speaker, decoded)
                }) },
                jpline: line,
                enline: translation
            });
        }
        drop(rows);
        drop(stmt);
        tx.commit()?;

        Ok(())
    }
}
