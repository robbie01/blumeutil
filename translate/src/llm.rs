mod similar;

pub use reqwest;

use std::fmt::Write as _;
use pyo3::Python;
use reqwest::Client;
use serde_json::{json, Value};
use anyhow::{anyhow, bail};

#[derive(Clone, Debug)]
struct Seen {
    speaker: String,
    orig: String,
    tled: String
}

fn build_prompt(seen: &[Seen], speaker: &str, orig: &str) -> anyhow::Result<String> {
    let mut prompt =
r#"<s>[INST] You are a translator working on the visual novel Edel Blume, a Japanese otome game. You will receive scripts in CSV format with the columns "Speaker" and "Dialogue" and translate them faithfully into English. [/INST]
Understood. I'll make sure that every line is translated accurately.</s>
[INST] Speaker,Dialogue
"#.to_owned();

    for &Seen { ref speaker, ref orig, ..} in seen.iter() {
        writeln!(prompt, "{speaker},{orig}")?;
    }
    writeln!(prompt, "{speaker},{orig} [/INST]")?;

    writeln!(prompt, "Speaker,Dialogue")?;
    for &Seen { ref speaker, ref tled, .. } in seen.iter() {
        writeln!(prompt, "{speaker},{tled}")?;
    }
    
    write!(prompt, "{speaker},")?;

    Ok(prompt)
}

#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    pub speaker: &'a str,
    pub orig: &'a str
}

/*
pub async fn translate<'a>(client: Client, it: impl IntoIterator<Item = Line<'a>>) -> anyhow::Result<Vec<String>> {
    let mut seen = Vec::new();

    for Line { speaker, orig } in it {
        eprintln!("{speaker}: {orig}");

        let control = google::translate(&client, orig).await?;
        eprintln!("control: {control}");

        let mut samples = Vec::new();

        let query = json!({
            "prompt": build_prompt(&seen, speaker, orig)?,
            "max_tokens": 200,
            "stop": "\n"
        });

        seen.push(Seen { speaker, orig, tled: loop {
            if samples.len() == 16 {
                let n = samples.len();
                let mut min = f64::MAX;
                let mut max = f64::MIN;
                let mut mean = 0.;

                for &s in samples.iter() {
                    if s < min { min = s; }
                    if s > max { max = s; }
                    mean += s;
                }
                mean /= n as f64;

                let mut stdev = 0.;

                for s in samples {
                    stdev += (s - mean)*(s - mean);
                }

                stdev = (stdev / (n - 1) as f64).sqrt();

                bail!("retry limit reached: min = {min}, max = {max}, mean = {mean}, stdev = {stdev}\ncontrol: {control}\n{speaker},{orig}\nseen = {seen:?}");
            }

            let r = client.post("http://127.0.0.1:5000/v1/completions").json(&query).send().await?.json::<Value>().await?;
            let tled = r.pointer("/choices/0/text").and_then(|s| s.as_str()).ok_or(anyhow!("couldn't get text! {r}"))?.trim();

            eprintln!("attempt {}: {tled}", samples.len() + 1);

            let score = Python::with_gil(|py| similar::similar(py, &control, tled))?;
            eprintln!("score: {score}");

            if score >= 0.75 {
                break tled.to_owned();
            }

            samples.push(score);
        } });
        eprintln!();
    }

    Ok(seen.into_iter().map(|s: Seen| s.tled).collect())
}
*/

#[derive(Clone, Debug)]
pub struct Translator {
    session: String,
    seen: Vec<Seen>
}

impl super::Translator for Translator {
    async fn translate(&mut self, client: Client, db: &rusqlite::Connection, script: u32, addr: u32) -> anyhow::Result<()> {
        todo!()
    }
}