use std::borrow::Cow;

use crate::Operation;
use encoding_rs::SHIFT_JIS;
// use xml::{EventWriter, writer::XmlEvent};

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
struct ParseState<'a> {
    addr: Option<u32>,
    speaker: String,
    options: Vec<Cow<'a, str>>,
    line: String
}

impl ParseState<'_> {
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

pub fn parse<'a>(it: impl IntoIterator<Item = Operation<'a>>) -> Vec<Dialogue> {
    use Operation::*;

    let mut st = ParseState::default();
    let mut di = Vec::new();
    for op in it {
        match op {
            Line(addr, s) => {
                if st.addr.is_none() { st.addr = Some(addr); }
                st.line.push_str(trim(&SHIFT_JIS.decode_without_bom_handling(s).0));
            },
            Choice(_addr, _, s) => {
                assert!(st.addr.is_some() && st.speaker.is_empty() && !st.line.is_empty(), "st = {st:#X?}");
                st.options.push(SHIFT_JIS.decode_without_bom_handling(s).0);
            },
            Speaker(addr, s) => {
                assert!(st.speaker.is_empty() && st.options.is_empty(), "st = {st:#X?}");
                if st.addr.is_none() { st.addr = Some(addr); }
                st.speaker.push_str(&SHIFT_JIS.decode_without_bom_handling(s).0);
            },
            _ => {
                if st.line.is_empty() {}
                else if !st.options.is_empty() {
                    assert!(st.addr.is_some() && st.speaker.is_empty(), "st = {st:#X?}");
                    di.push(Dialogue::Choice { addr: st.addr.unwrap(), prompt: st.line.clone(), options: st.options.iter().map(|s| s.clone().into_owned()).collect() })
                } else {
                    assert!(st.addr.is_some() && st.options.is_empty(), "st = {st:#X?}");
                    di.push(Dialogue::Line { addr: st.addr.unwrap(), speaker: (!st.speaker.is_empty()).then(|| st.speaker.clone()), line: st.line.clone() })
                }
                st.clear();
            }
        }
    }

    di
}

// #[allow(unused)]
// pub fn to_xml(writer: &mut EventWriter<impl Write>, it: impl IntoIterator<Item = Dialogue>) -> anyhow::Result<()> {
//     use Dialogue::*;
//     let mut buf = String::new();
//     writer.write(XmlEvent::start_element("scene").attr("id", "00000064"))?;
//     for d in it {
//         match d {
//             Choice { addr, prompt, options } => {
//                 buf.clear();
//                 write!(buf, "{addr:X}")?;
//                 writer.write(XmlEvent::start_element("choice").attr("addr", &buf))?;
//                 writer.write(XmlEvent::start_element("prompt"))?;
//                 writer.write(XmlEvent::characters(&prompt))?;
//                 writer.write(XmlEvent::end_element())?;
//                 for opt in options {
//                     writer.write(XmlEvent::start_element("option"))?;
//                     writer.write(XmlEvent::characters(&opt))?;
//                     writer.write(XmlEvent::end_element())?;
//                 }
//                 writer.write(XmlEvent::end_element())?;
//             },
//             Line { addr, speaker, line } => {
//                 buf.clear();
//                 write!(buf, "{addr:X}")?;
//                 writer.write(XmlEvent::start_element("line").attr("addr", &buf))?;
//                 if let Some(speaker) = speaker {
//                     writer.write(XmlEvent::start_element("speaker"))?;
//                     writer.write(XmlEvent::characters(&speaker))?;
//                     writer.write(XmlEvent::end_element())?;
//                 }
//                 writer.write(XmlEvent::start_element("dialogue"))?;
//                 writer.write(XmlEvent::characters(&line))?;
//                 writer.write(XmlEvent::end_element())?;
//                 writer.write(XmlEvent::end_element())?;
//             }
//         }
//     }
//     writer.write(XmlEvent::end_element())?;

//     Ok(())
// }