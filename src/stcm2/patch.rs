use std::{collections::{BTreeMap, HashMap}, iter, mem, slice};

use anyhow::{bail, ensure};
use bytes::{BufMut as _, Bytes, BytesMut};
use encoding_rs::SHIFT_JIS;
use once_cell::sync::Lazy;
use rusqlite::Connection;
use crate::stcm2::format::Address;

use super::{format::{self, Action, Parameter, Stcm2}, Args};

const MAX_LINE_LENGTH: usize = 45; // game will print a debug message if the line is over 45 halfwidth chars

fn encode_string(enc: &[u8]) -> anyhow::Result<BytesMut> {
    let qlen = enc.len().div_ceil(4);
    let mut b = BytesMut::new();
    b.put_u32_le(0);
    b.put_u32_le(qlen.try_into()?);
    b.put_u32_le(1);
    b.put_u32_le((qlen * 4).try_into()?);
    b.put_slice(enc);
    b.put_bytes(0, (qlen * 4) - enc.len());

    Ok(b)
}

// trust me on this
static SPEAKERS: Lazy<HashMap<&[u8], Bytes>> = Lazy::new(|| HashMap::from([
    (&b"#Name[1]"[..], encode_string(b"#Name[1]").unwrap().freeze()),
    (b"\x81H\x81H\x81H", encode_string(b"\x81H\x81H\x81H").unwrap().freeze()), // ???
    (b"\x83_\x83j\x83G\x83\x89", encode_string(b"Daniela").unwrap().freeze()),
    (b"\x83\x94\x83B\x83N\x83g\x83\x8b", encode_string(b"Victor").unwrap().freeze()),
    (b"\x83I\x81[\x83M\x83\x85\x83X\x83g", encode_string(b"Auguste").unwrap().freeze()),
    (b"\x83C\x83\x8a\x83\x84", encode_string(b"Ilya").unwrap().freeze()),
    (b"\x83\x8a\x83`\x83\x83\x81[\x83h", encode_string(b"Richard").unwrap().freeze()),
    (b"\x83\x84\x83R\x83u", encode_string(b"Jacob").unwrap().freeze()),
    (b"\x83o\x81[\x83W\x83j\x83A", encode_string(b"Virginia").unwrap().freeze()),
    (b"\x83W\x83F\x83\x89\x83\x8b\x83h", encode_string(b"Gerald").unwrap().freeze()),
    (b"\x83R\x83\x93\x83\x89\x83b\x83h", encode_string(b"Conrad").unwrap().freeze()),
    (b"\x83o\x83\x89\x81[\x83W\x83\x85", encode_string(b"Balazs").unwrap().freeze()),
    (b"\x83N\x83\x89\x83E\x83X", encode_string(b"Claus").unwrap().freeze()),
    (b"\x83X\x83e\x83t\x83@\x83\x93", encode_string(b"Stefan").unwrap().freeze()),
    (b"\x83\x8c\x83\x8b\x83\x80", encode_string(b"Larm").unwrap().freeze()),
    (b"\x83\x8c\x83I", encode_string(b"Leo").unwrap().freeze()),
    (b"\x83M\x83\x8b\x83x\x83\x8b\x83g", encode_string(b"Gilbert").unwrap().freeze()),
    (b"\x83G\x83~\x83\x8a\x83I", encode_string(b"Emilio").unwrap().freeze()),
    (b"\x83f\x83B\x83\x81\x83g\x83\x8a\x83I", encode_string(b"Demetrio").unwrap().freeze()),
    (b"\x83I\x83\x8a\x83\x94\x83B\x83A", encode_string(b"Olivia").unwrap().freeze()),
    (b"\x83\x94\x83H\x83\x8b\x83}\x81[", encode_string(b"Volmer").unwrap().freeze()),
    (b"\x83_\x83\x93\x83P\x83\x8b\x83n\x83C\x83g", encode_string(b"Dunkelheit").unwrap().freeze()),
    (b"\x90l\x98T", encode_string(b"Werewolf").unwrap().freeze()),
    (b"\x8d\x95\x88\xdf\x82\xcc\x92j\x90\xab", encode_string(b"Man in black").unwrap().freeze()),
    (b"\x83N\x83\x89\x83E\x83X\x82\xcc\x90\xba", encode_string(b"Claus's voice").unwrap().freeze()),
    (b"\x83o\x81[\x83W\x83j\x83A\x82\xcc\x90\xba", encode_string(b"Virginia's voice").unwrap().freeze()),
    (b"\x83I\x81[\x83M\x83\x85\x83X\x83g\x82\xcc\x90\xba", encode_string(b"Auguste's voice").unwrap().freeze()),
    (b"\x83X\x83e\x83t\x83@\x83\x93\x82\xcc\x90\xba", encode_string(b"Stefan's voice").unwrap().freeze())
]));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Fullwidth([u8; 2]),
    Halfwidth(u8),
    Name
}

impl Token {
    fn width(self) -> usize {
        match self {
            Self::Fullwidth(_) => 2,
            Self::Halfwidth(_) => 1,
            Self::Name => 10
        }
    }

    fn rep(&self) -> &[u8] {
        match *self {
            Self::Fullwidth(ref c) => c,
            Self::Halfwidth(ref c) => slice::from_ref(c),
            Self::Name => b"#Name[1]"
        }
    }
}

fn tokenize(mut input: &[u8]) -> impl Iterator<Item = Token> + '_ {
    iter::from_fn(move || {
        if input.is_empty() {
            None
        } else if let Some(sl) = input.strip_prefix(b"#Name[1]") {
            input = sl;
            Some(Token::Name)
        } else if matches!(input[0], 0x81..=0x9F | 0xE0..=0xFC) {
            let &[ch, cl, ref sl @ ..] = input else { panic!("sjis terminates early") };
            input = sl;
            Some(Token::Fullwidth([ch, cl]))
        } else {
            let &[c, ref sl @ ..] = input else { unreachable!() };
            input = sl;
            Some(Token::Halfwidth(c))
        }
    })
}

fn split_lines_intelligent(input: &[Token]) -> Vec<Vec<u8>> {
    const FULLWIDTH_SPACE: [u8; 2] = [0x81, 0x40];

    let mut v = Vec::new();
    let mut cur_width = 0;
    let mut cur_line = Vec::default();
    let mut indent = false;
    for atom in input.split(|&t| t == Token::Halfwidth(b' ')) {
        loop {
            if cur_width == 0 {
                if indent {
                    cur_line.extend_from_slice(&FULLWIDTH_SPACE);
                    cur_width += 2;
                }
                for &tok in atom.iter() {
                    cur_line.extend_from_slice(tok.rep());
                    cur_width += tok.width();
                }
                if matches!(atom[0], Token::Fullwidth(_)) {
                    indent = true;
                }
            } else {
                let width = atom.iter().map(|&tok| tok.width()).sum::<usize>();
                if cur_width + width + 1 > MAX_LINE_LENGTH {
                    assert!(cur_width <= MAX_LINE_LENGTH);
                    v.push(mem::take(&mut cur_line));
                    cur_width = 0;
                    continue;
                }
                cur_line.push(b' ');
                cur_width += 1;
                for &tok in atom.iter() {
                    cur_line.extend_from_slice(tok.rep());
                    cur_width += tok.width();
                }
            }

            break;
        }
    }
    if cur_width > 0 {
        assert!(cur_width <= MAX_LINE_LENGTH);
        v.push(cur_line);
    }
    v
}

pub fn patch(mut db: Connection, args: Args) -> anyhow::Result<()> {
    let tx = db.transaction()?;

    let mut tls = HashMap::new();
    {
        let mut stmt = tx.prepare("SELECT address, translation FROM translations WHERE session = 'vntl-greedy-20240220' AND scriptid = ?")?;
        let mut rows = stmt.query((args.id,))?;
        while let Some(row) = rows.next()? {
            let (address, translation) = <(u32, String)>::try_from(row)?;
            tls.insert(address, translation);
        }
    }

    let file = tx.query_row("SELECT script FROM scripts WHERE id = ?", (args.id,), |row| Ok(Bytes::copy_from_slice(row.get_ref(0)?.as_blob()?)))?;

    let stcm2 = format::from_bytes(file)?;

    let mut cur_addr = None;
    let mut new_actions = BTreeMap::new();
    let mut buf_actions = BTreeMap::new();

    for (addr, act) in stcm2.actions {
        match act {
            Action { call: false, opcode: Action::OP_SPEAKER, ref export, ref params, .. } => {
                ensure!(cur_addr.is_none() && buf_actions.is_empty() && export.is_none() && matches!(&params[..], &[Parameter::LocalPointer(0)]));
                let name = format::decode_string(0, act.data)?;
                let Some(speaker) = SPEAKERS.get(&name[..]) else { bail!("could not get speaker for {:?}", name) };
                new_actions.insert(addr, Action {
                    data: speaker.clone(),
                    ..act
                });
                cur_addr = Some(Address {
                    orig: addr.orig,
                    sub: addr.sub + 1
                })
            },
            Action { call: false, opcode: Action::OP_LINE, ref export, ref params, .. } => {
                match params[..] {
                    [Parameter::Value(212)] => {
                        // idk
                        ensure!(cur_addr.is_none());
                        new_actions.insert(addr, act);
                        continue;
                    },
                    [Parameter::LocalPointer(0)] => (),
                    _ => bail!("{params:?}")
                }
                ensure!(export.is_none());
                buf_actions.insert(addr, act);
                if cur_addr.is_none() {
                    cur_addr = Some(addr);
                }
            },
            act => {
                if act.opcode != Action::OP_CHOICE {
                    if let Some(mut addr) = cur_addr {
                        if let Some(mut translation) = tls.remove(&addr.orig) {
                            const REPLACE: &[(&str, &str)] = &[
                                // this should be in the db already :/
                                ("Mary", "#Name[1]"),

                                ("ä", "a"),

                                // fullwidth -> halfwidth
                                //("「", "｢"),
                                //("」", "｣"),
                                // ("（", "("),
                                // ("）", ")")
                            ];

                            buf_actions.clear();

                            for &(from, to) in REPLACE.iter() {
                                translation = translation.replace(from, to);
                            }

                            let (translation, _, false) = SHIFT_JIS.encode(&translation) else { bail!("found invalid sjis chars") };

                            let lines = {
                                let tokens = tokenize(&translation).collect::<Vec<_>>();
                                let lines = split_lines_intelligent(&tokens);
                                if lines.len() > 3 {
                                    eprintln!("warning: split went overlong");
                                }
                                lines
                            };

                            let mut yield_counter = 0;

                            for line in lines {
                                if yield_counter >= 3 {
                                    yield_counter = 0;
                                    new_actions.insert(addr, Action {
                                        export: None,
                                        call: false,
                                        opcode: Action::OP_YIELD,
                                        params: Vec::new(),
                                        data: Bytes::new()
                                    });
                                    addr.sub += 1;
                                }

                                new_actions.insert(addr, Action {
                                    export: None,
                                    call: false,
                                    opcode: Action::OP_LINE,
                                    params: vec![Parameter::LocalPointer(0)],
                                    data: encode_string(&line)?.freeze()
                                });
                                addr.sub += 1;
                                yield_counter += 1;
                            }
                        } else {
                            new_actions.append(&mut buf_actions);
                        }
                    } else {
                        ensure!(buf_actions.is_empty());
                    }
                } else {
                    new_actions.append(&mut buf_actions);
                }
                ensure!(buf_actions.is_empty());
                new_actions.insert(addr, act);
                cur_addr = None;
            }
        }
    }
    ensure!(tls.is_empty() && buf_actions.is_empty() && cur_addr.is_none());

    let stcm2 = Stcm2 {
        actions: new_actions,
        ..stcm2
    };

    let refile = format::to_bytes(stcm2)?;

    tx.execute("INSERT OR REPLACE INTO patchedscripts(id, script) VALUES (?, ?)", (args.id, &refile[..]))?;

    tx.commit()?;

    Ok(())
}
