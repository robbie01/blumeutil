use std::mem;

use super::Operation;
use anyhow::ensure;
use encoding_rs::SHIFT_JIS;

#[derive(Clone, Debug)]
pub enum Dialogue {
    Choice {
        addr: u32,
        prompt: String,
        options: Vec<String>
    },
    Line {
        addr: u32,
        speaker: String,
        line: String
    }
}

#[derive(Clone, Debug, Default)]
struct ParseState {
    addr: Option<u32>,
    speaker: String,
    options: Vec<String>,
    line: String
}

fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '・')
}

fn decode(b: &[u8]) -> String {
    let (dec, rep) = SHIFT_JIS.decode_without_bom_handling(b);
    assert!(!rep, "replacements were made in {dec}");
    dec.into_owned()
}

pub fn parse(it: impl IntoIterator<Item = Operation>) -> anyhow::Result<Vec<Dialogue>> {
    use Operation::*;

    let mut st = ParseState::default();
    let mut di = Vec::new();
    for op in it {
        match op {
            Line(addr, s) => {
                ensure!(st.options.is_empty(), "incorrect line state\nst = {st:#X?}");
                if st.addr.is_none() { st.addr = Some(addr); }
                st.line.push_str(trim(&decode(&s)));
            },
            Choice(addr, _, s) => {
                if st.addr.is_none() { st.addr = Some(addr); }
                st.options.push(decode(&s));
            },
            Speaker(addr, s) => {
                ensure!(st.speaker.is_empty() && st.options.is_empty(), "incorrect speaker state\nst = {st:#X?}");
                if st.addr.is_none() { st.addr = Some(addr); }
                st.speaker.push_str(&decode(&s));
            },
            _ => {
                let ParseState { addr, speaker, options, line } = mem::take(&mut st);
                if line.is_empty() {}
                else if !options.is_empty() {
                    ensure!(addr.is_some());
                    di.push(Dialogue::Choice { addr: addr.unwrap(), prompt: line, options })
                } else {
                    ensure!(addr.is_some() && options.is_empty());
                    di.push(Dialogue::Line { addr: addr.unwrap(), speaker, line })
                }
            }
        }
    }

    Ok(di)
}
