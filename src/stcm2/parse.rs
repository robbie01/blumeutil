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
        speaker: Option<String>,
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

impl ParseState {
    fn clear(&mut self) {
        self.addr = None;
        self.speaker.clear();
        self.options.clear();
        self.line.clear();
    }
}

fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '・')
}

fn decode(b: &[u8]) -> String {
    SHIFT_JIS.decode_without_bom_handling(b).0.into_owned()
}

pub fn parse(it: impl IntoIterator<Item = Operation>) -> anyhow::Result<Vec<Dialogue>> {
    use Operation::*;

    let mut st = ParseState::default();
    let mut di = Vec::new();
    for op in it {
        match op {
            Line(addr, s) => {
                if st.addr.is_none() { st.addr = Some(addr); }
                st.line.push_str(trim(&decode(&s)));
            },
            Choice(_addr, _, s) => {
                ensure!(st.addr.is_some() && !st.line.is_empty(), "st = {st:#X?}");
                st.options.push(decode(&s));
            },
            Speaker(addr, s) => {
                ensure!(st.speaker.is_empty() && st.options.is_empty(), "st = {st:#X?}");
                if st.addr.is_none() { st.addr = Some(addr); }
                st.speaker.push_str(&decode(&s));
            },
            _ => {
                if st.line.is_empty() {}
                else if !st.options.is_empty() {
                    ensure!(st.addr.is_some(), "st = {st:#X?}");
                    di.push(Dialogue::Choice { addr: st.addr.unwrap(), prompt: st.line.clone(), options: st.options.iter().map(|s| s.clone()).collect() })
                } else {
                    ensure!(st.addr.is_some() && st.options.is_empty(), "st = {st:#X?}");
                    di.push(Dialogue::Line { addr: st.addr.unwrap(), speaker: (!st.speaker.is_empty()).then(|| st.speaker.clone()), line: st.line.clone() })
                }
                st.clear();
            }
        }
    }

    Ok(di)
}
