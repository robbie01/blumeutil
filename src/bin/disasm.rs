use std::{borrow::Cow, cmp::Ordering, collections::{BTreeMap, btree_map}, fmt::Write as _, path::PathBuf, str};
use anyhow::{anyhow, bail, ensure, Context as _};
use bytes::{Buf as _, Bytes, BytesMut};
use clap::Parser;
use rusqlite::Connection;
use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use encoding_rs::SHIFT_JIS;

const STCM2_MAGIC: &[u8] = b"STCM2 File Make By Minku 07.0\0\0\0";
const GLOBAL_DATA_MAGIC: &[u8] = b"GLOBAL_DATA\0\0\0\0\0";
const GLOBAL_DATA_OFFSET: usize = STCM2_MAGIC.len() + 12*4 + GLOBAL_DATA_MAGIC.len();
const CODE_START_MAGIC: &[u8] = b"CODE_START_\0";
const EXPORT_DATA_MAGIC: &[u8] = b"EXPORT_DATA\0";

#[derive(Clone, Copy, Debug)]
enum Parameter {
    GlobalPointer(u32),
    LocalPointer(u32),
    Value(u32)
}

impl Parameter {
    fn parse(value: [u32; 3], data_addr: u32, data_len: u32) -> anyhow::Result<Self> {
        match value {
            [0xffffff41, addr, 0xff000000] => Ok(Self::GlobalPointer(addr)),
            [addr, 0xff000000, 0xff000000]
                if (addr & 0xff000000 != 0xff000000) && addr >= data_addr && addr < data_addr+data_len
                => Ok(Self::LocalPointer(addr-data_addr)),
            [value, 0xff000000, 0xff000000] => Ok(Self::Value(value)),
            _ => Err(anyhow!("bad parameter: {value:08X?}"))
        }
    }
}

#[derive(Clone, Debug)]
struct Action {
    export: Option<Bytes>,
    call: bool,
    opcode: u32,
    params: Vec<Parameter>,
    data: Bytes
}

fn decode_string(addr: u32, mut str: Bytes) -> anyhow::Result<(Bytes, Bytes)> {
    str.advance(addr as usize);

    ensure!(str.get_u32_le() == 0, "string magic isn't 0");
    let qlen = str.get_u32_le();
    ensure!(str.get_u32_le() == 1, "string magic isn't 1");
    let len = str.get_u32_le();
    ensure!(len/4 == qlen, "len and qlen are inconsistent: len = {len}, qlen = {qlen}");

    let tail = str.split_off(len.try_into()?);

    // clip zeros off end
    while let Some(0) = str.last() {
        str.truncate(str.len()-1);
    }

    Ok((str, tail))
}

#[allow(dead_code)]
impl Action {
    const OP_SPEAKER: u32 = 0xd4;
    const OP_YIELD: u32 = 0xd3;
    const OP_LINE: u32 = 0xd2;
    const OP_CHOICE: u32 = 0xe7;

    const OP_ADD: u32 = 0xffffff00;
    //const OP_SUB: u32 = 0xffffff01;
    const OP_MUL: u32 = 0xffffff02;
    //const OP_DIV: u32 = 0xffffff03;
    //const OP_MOD: u32 = 0xffffff04;
    //const OP_SHL: u32 = 0xffffff05;
    //const OP_SHR: u32 = 0xffffff06;
    //const OP_AND: u32 = 0xffffff07;
    //const OP_XOR: u32 = 0xffffff08;
    //const OP_OR: u32 = 0xffffff09;

    fn label(&self) -> Option<&str> {
        self.export.as_ref().and_then(|e| str::from_utf8(e).ok()).map(|s| s.trim_end_matches('\0'))
    }
}

#[derive(Clone, Debug)]
struct Stcm2 {
    global_data: Bytes,
    actions: BTreeMap<u32, Action>
}

fn autolabel(addr: u32) -> anyhow::Result<Bytes> {
    let mut l = BytesMut::new();
    write!(l, "local_{addr:X}")?;
    Ok(l.freeze())
}

fn from_bytes(mut file: Bytes) -> anyhow::Result<Stcm2> {
    let start_addr = file.as_ptr();
    let get_pos = |file: &Bytes| file.as_ptr() as usize - start_addr as usize;

    ensure!(file.starts_with(STCM2_MAGIC));
    file.advance(STCM2_MAGIC.len());
    let export_addr = file.get_u32_le();
    let export_len = file.get_u32_le();
    for _ in 0..10 {
        ensure!(file.get_u32_le() == 0);
    }
    ensure!(file.starts_with(GLOBAL_DATA_MAGIC));
    file.advance(GLOBAL_DATA_MAGIC.len());
    ensure!(get_pos(&file) == GLOBAL_DATA_OFFSET);
    let mut global_len = 0;
    while !file[global_len..].starts_with(CODE_START_MAGIC) {
        global_len += 16;
    }
    let global_data = file.split_to(global_len);
    ensure!(file.starts_with(CODE_START_MAGIC));
    file.advance(CODE_START_MAGIC.len());

    let mut actions = BTreeMap::new();

    while get_pos(&file) < usize::try_from(export_addr)? - EXPORT_DATA_MAGIC.len() {
	    let addr = get_pos(&file).try_into()?;
		
        let global_call = file.get_u32_le();
        let opcode = file.get_u32_le();
        let nparams = file.get_u32_le();
        let length = file.get_u32_le();

        let call = match global_call {
            0 => false,
            1 => true,
            v => bail!("global_call = {v:08X}")
        };
        let mut params = Vec::with_capacity(nparams.try_into()?);
        for _ in 0..nparams {
            let buffer = [file.get_u32_le(), file.get_u32_le(), file.get_u32_le()];
            params.push(Parameter::parse(buffer, addr + 16 + 12*nparams, length - 16 - 12*nparams)?);
        }

        let ndata = length - 16 - 12*nparams;
        let data = file.split_to(ndata.try_into()?);

        let res = actions.insert(addr, Action { export: None, call, opcode, params, data });
        ensure!(res.is_none());
    }

    ensure!(file.starts_with(EXPORT_DATA_MAGIC));
    file.advance(EXPORT_DATA_MAGIC.len());

    for _ in 0..export_len {
        ensure!(file.get_u32_le() == 0);
        let export = file.split_to(32);
        let addr = file.get_u32_le();
        let act = actions.get_mut(&addr).context("export does not match known action")?;
        ensure!(act.export.is_none());
        act.export = Some(export);
    }

    Ok(Stcm2 {
        global_data,
        actions
    })
}

#[derive(Parser)]
struct Args {
    file: PathBuf,
    id: u32
}

fn decode_sjis(buf: &[u8]) -> anyhow::Result<Cow<'_, str>> {
    SHIFT_JIS.decode_without_bom_handling_and_without_replacement(buf).context("unknown character hit")
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let db = Connection::open(args.file)?;
    let file = db.query_row("SELECT script FROM patchedscripts WHERE id = ?", (args.id,),
        |row| Ok(Bytes::copy_from_slice(row.get_ref(0)?.as_blob()?)))?;
    let mut stcm2 = from_bytes(file)?;

    //let s = stcm2.actions.values().map(|act| act.opcode).collect::<std::collections::BTreeSet<u32>>();
    //println!("{s:#X?}");

    //return Ok(());

    // build symbol table and autolabels
    let mut labels = BTreeMap::new();
    for act in stcm2.actions.values() {
        if let Action { call: true, opcode, .. } = *act {
            if stcm2.actions.get(&opcode).context("bruh0")?.export.is_none() {
                if let btree_map::Entry::Vacant(entry) = labels.entry(opcode) {
                    entry.insert(autolabel(opcode)?);
                }
            }
        }
        for &param in act.params.iter() {
            if let Parameter::GlobalPointer(addr) = param {
                if stcm2.actions.get(&addr).context("bruh9")?.export.is_none() {
                    if let btree_map::Entry::Vacant(entry) = labels.entry(addr) {
                        entry.insert(autolabel(addr)?);
                    }
                }
            }
        }
    }
    if let Some(((&begin, _), (&end, _))) = labels.first_key_value().zip(labels.last_key_value()) {
        let mut acts = stcm2.actions.range_mut(begin..=end);
        for (addr, label) in labels {
            let act = loop {
                let (&k, v) = acts.next().context("this should never happen 1")?;
                match k.cmp(&addr) {
                    Ordering::Less => (),
                    Ordering::Equal => break v,
                    Ordering::Greater => bail!("this should never happen 2")
                }
            };
            ensure!(act.export.is_none());
            act.export = Some(label);
        }
    }

    println!(".tag \"{}\"", str::from_utf8(&STCM2_MAGIC[5..]).context("nooooo")?.trim_end_matches('\0'));
    println!(".global_data {}", Base64Display::new(&stcm2.global_data, &STANDARD));
    println!(".code_start");
    for act in stcm2.actions.values() {
        if let Some(label) = act.label() {
            print!("{label}: ");
        }
        match *act {
            Action { call: true, opcode, ref params, ref data, .. } => {
                print!("call {}", stcm2.actions.get(&opcode).context("bruh")?.label().context("bruh2")?);
                for &param in params.iter() {
                    match param {
                        Parameter::Value(v) => print!(", {v:X}"),
                        Parameter::GlobalPointer(addr) => print!(", [{}]", stcm2.actions.get(&addr).context("bruh3")?.label().context("bruh4")?),
                        Parameter::LocalPointer(addr) => print!(", [data+{addr}]")
                    }
                }

                if !data.is_empty() {
                    print!(", {}", Base64Display::new(data, &STANDARD));
                }
            },
            Action { opcode: Action::OP_YIELD, ref params, ref data, .. } if params.is_empty() && data.is_empty() => {
                print!("yield");
            },
            Action { opcode: Action::OP_SPEAKER, ref params, ref data, .. } if matches!(params[..], [Parameter::LocalPointer(0)]) => {
                let (s, tail) = decode_string(0, data.clone())?;
                ensure!(tail.is_empty());
                print!("speaker \"{}\"", decode_sjis(&s)?);
            },
            Action { opcode: Action::OP_LINE, ref params, ref data, .. } if matches!(params[..], [Parameter::LocalPointer(0)]) => {
                let (s, tail) = decode_string(0, data.clone())?;
                ensure!(tail.is_empty());
                print!("line \"{}\"", decode_sjis(&s)?);
            },
            Action { opcode: Action::OP_CHOICE, ref params, ref data, .. } if matches!(params[..], [Parameter::LocalPointer(0), Parameter::Value(v)] if v & 0xFF000000 == 0xFF000000) => {
                let [Parameter::LocalPointer(0), Parameter::Value(v)] = params[..] else { unreachable!() };
                let (s, tail) = decode_string(0, data.clone())?;
                ensure!(tail.is_empty());
                print!("choice {:X}, \"{}\"", v & !0xFF000000, decode_sjis(&s)?);
            },
            Action { opcode, ref params, ref data, .. } => {
                print!("raw {opcode:X}");
                for &param in params.iter() {
                    match param {
                        Parameter::Value(v) => print!(", {v:X}"),
                        Parameter::GlobalPointer(addr) => print!(", [{}]", stcm2.actions.get(&addr).context("bruh5")?.label().context("bruh6")?),
                        Parameter::LocalPointer(addr) => print!(", [data+{addr}]")
                    }
                }

                'printdata: {
                    if params.iter().map(|p| match p { Parameter::LocalPointer(0) => 1, _ => 0 }).sum::<usize>() == 1 {
                        if let Ok((s, tail)) = decode_string(0, data.clone()) {
                            if tail.is_empty() {
                                if let Some(s) = SHIFT_JIS.decode_without_bom_handling_and_without_replacement(&s) {
                                    print!(", \"{s}\"");
                                    break 'printdata; // basically a goto
                                }
                            }
                        }
                    }
                    if !data.is_empty() {
                        print!(", {}", Base64Display::new(data, &STANDARD));
                    }
                }
            }
        }
        println!();
    }

    Ok(())
}
