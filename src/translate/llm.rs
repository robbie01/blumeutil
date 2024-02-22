mod characters;

use std::{collections::HashSet, fmt::{Display, Write as _}, num::NonZeroU32};

use rusqlite::{Connection, DropBehavior};
use llama::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{
        AddBos,
        LlamaModel,
        params::LlamaModelParams
    },
    token::{
        LlamaToken,
        data_array::LlamaTokenDataArray
    }
};
use once_cell::sync::Lazy;

use characters::{decode_jp_speaker, Character, EnSpeaker};

static LLAMA_BACKEND: Lazy<LlamaBackend> = Lazy::new(|| LlamaBackend::init().unwrap());

const TOKENS_RESERVED: u16 = 256;

#[derive(Debug)]
pub struct Translator {
    session: String,
    llm: LlamaModel
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
        write!(f, "{}<|endoftext|>", &self.enline)?;

        Ok(())
    }
}

fn build_header(seen: &[Seen], next_speaker: Option<&str>) -> anyhow::Result<String> {
    let cs = seen.iter()
        .filter_map(|s| s.speaker.as_ref())
        .map(|(j, _)| j.as_str())
        .chain(next_speaker)
        .filter_map(|j| match decode_jp_speaker(j) {
            Ok(EnSpeaker::Str(_)) => None,
            Ok(EnSpeaker::Character(c)) => Some(Ok(c)),
            Err(e) => Some(Err(e))
        })
        .collect::<anyhow::Result<HashSet<&Character>>>()?;
    let mut header = "<<METADATA>>\n".to_owned();
    for c in cs {
        write!(header, "[character] {c}\n")?;
    }
    write!(header, "<<START>>\n")?;

    Ok(header)
}

fn build_prompt(seen: &[Seen], next_speaker: Option<&str>, next_line: &str) -> anyhow::Result<String> {
    let mut prompt = build_header(seen, next_speaker)?;
    for s in seen {
        write!(prompt, "{s}\n")?;
    }
    prompt.push_str("<<JAPANESE>>\n");
    if let Some(next_speaker) = next_speaker {
        write!(prompt, "[{next_speaker}]: ")?;
    }
    write!(prompt, "{next_line}\n<<ENGLISH>>\n")?;
    if let Some(enspeaker) = next_speaker.map(decode_jp_speaker).transpose()? {
        write!(prompt, "[{enspeaker}]: ")?;
    }

    Ok(prompt)
}

#[derive(Clone, Debug)]
struct MaxTokensReachedError(Vec<LlamaToken>);

impl Display for MaxTokensReachedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("maximum number of tokens reached")
    }
}

impl std::error::Error for MaxTokensReachedError {}

impl Translator {
    pub fn new(session: String) -> anyhow::Result<Self> {
        let llm = LlamaModel::load_from_file(
            &LLAMA_BACKEND,
            "/home/robbie/models/vntl-13b-v0.2-Q4_K_M.gguf", 
            &LlamaModelParams::default()
                .with_n_gpu_layers(if cfg!(feature = "cuda") {
                    i32::MAX as u32
                } else {
                    0
                })
        )?;

        Ok(Self { session, llm })
    }

    fn get_completion(&self, prompt: &[LlamaToken], n_ctx: u16) -> anyhow::Result<String> {
        let mut ctx = self.llm.new_context(
            &LLAMA_BACKEND,
            LlamaContextParams::default()
                .with_seed(1234567890)
                .with_n_ctx(NonZeroU32::new(n_ctx.into()))
        )?;

        let n_batch = ctx.n_batch().try_into()?;
        let mut batch = LlamaBatch::new(n_batch, 1);

        let mut chunks = prompt.chunks(n_batch).peekable();
        let mut pos = 0;
        while let Some(chunk) = chunks.next() {
            batch.clear();
            let mut tokens = chunk.iter().peekable();

            while let Some(&token) = tokens.next() {
                let last = chunks.peek().is_none() && tokens.peek().is_none();
                batch.add(token, pos, &[0], last)?;
                pos += 1
            }

            ctx.decode(&mut batch)?;
        }

        let mut tokens = Vec::new();
        loop {
            let token = ctx.sample_token_greedy(
                LlamaTokenDataArray::from_iter(
                    ctx.candidates_ith(batch.n_tokens() - 1),
                    false
                )
            );
            if token == self.llm.token_eos() {
                eprint!("\n\n");
                break;
            }
            eprint!("{}", self.llm.token_to_str(token)?);
            tokens.push(token);
            if tokens.len() >= TOKENS_RESERVED.into() {
                eprintln!();
                return Err(MaxTokensReachedError(tokens).into());
            }
            batch.clear();
            batch.add(token, pos, &[0], true)?;
            pos += 1;
            ctx.decode(&mut batch)?;
        }

        Ok(self.llm.tokens_to_str(&tokens)?.trim().to_owned())
    }

    pub fn translate(&self, db: &mut Connection, script: u32) -> anyhow::Result<()> {
        let n_ctx = self.llm.n_ctx_train().min(4096);

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
        ")?;

        let mut rows = stmt.query((&self.session, script))?;
        while let Some(row) = rows.next()? {
            let (address, mut speaker, mut line, translation) = <(u32, String, String, Option<String>)>::try_from(row)?;
            if speaker == "#Name[1]" {
                speaker = "メアリ".to_owned();
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

            let prompt = loop {
                let strprompt = build_prompt(&seen, (!speaker.is_empty()).then_some(&speaker), &line)?;
                let prompt = self.llm.str_to_token(&strprompt, AddBos::Always)?;
                if prompt.len() > (n_ctx - TOKENS_RESERVED).into() {
                    seen.remove(0);
                    continue;
                }
                break prompt;
            };

            eprintln!("address = {address}");
            eprintln!("{line}");
            let translation = self.get_completion(&prompt, n_ctx)?;
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
