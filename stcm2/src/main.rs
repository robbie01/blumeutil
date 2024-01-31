mod parse;
mod dict;

use std::{collections::HashMap, io::{BufReader, SeekFrom, ErrorKind, Read as _, Seek as _, BufRead as _}, path::PathBuf};
use anyhow::{bail, ensure, anyhow};
use clap::Parser;
use byteorder::{LittleEndian as LE, ReadBytesExt as _};
use encoding_rs::SHIFT_JIS;
use rusqlite::{Connection, DatabaseName, DropBehavior};

const STCM2_MAGIC: &[u8] = b"STCM2 File Make By Minku 07.0\0\0\0";
const CODE_START_MAGIC: &[u8] = b"CODE_START_\0";

#[derive(Parser)]
struct Args {
    #[arg(help = "path to the SQLite database")]
    db: PathBuf,
    #[arg(help = "id of the script to analyze")]
    script_id: u32
}

#[derive(Clone, Copy)]
pub enum Operation<'a> {
    Speaker(u32, &'a [u8]),
    Line(u32, &'a [u8]),
    Choice(u32, u32, &'a [u8]),
    Yield(u32),
    Raw(&'a Action)
}

impl std::fmt::Debug for Operation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Operation::*;

        match *self {
            Yield(ref addr) => f.debug_tuple("Yield").field(addr).finish(),
            Speaker(ref addr, s) => f.debug_tuple("Speaker").field(addr).field(&SHIFT_JIS.decode_without_bom_handling(s).0).finish(),
            Line(ref addr, s) => f.debug_tuple("Line").field(addr).field(&SHIFT_JIS.decode_without_bom_handling(s).0).finish(),
            Choice(ref addr, ref i, s) => f.debug_tuple("Choice").field(addr).field(i).field(&SHIFT_JIS.decode_without_bom_handling(s).0).finish(),
            Raw(act) => f.debug_tuple("Raw").field(act).finish()
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Export {
    name: [u8; 32],
    addr: u64
}

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
            [addr, 0xff000000, 0xff000000] if (addr & 0xff000000 != 0xff000000) && addr >= data_addr && addr < data_addr+data_len => Ok(Self::LocalPointer(addr-data_addr)),
            [value, 0xff000000, 0xff000000] => Ok(Self::Value(value & !0xff000000)),
            _ => Err(anyhow!("bad parameter: {value:08X?}"))
        }
    }
}

#[derive(Clone, Debug)]
pub struct Action {
    addr: u32,
    #[allow(unused)]
    export: Option<[u8; 32]>,
    call: bool,
    opcode: u32,
    params: Vec<Parameter>,
    data: Vec<u8>
}

fn decode_string(mut str: &[u8]) -> anyhow::Result<&[u8]> {
    if str.read_u32::<LE>()? != 0 { bail!("string magic isn't 0"); }
    let qlen = str.read_u32::<LE>()?;
    if str.read_u32::<LE>()? != 1 { bail!("string magic isn't 1"); }
    let len = str.read_u32::<LE>()?;
    if len/4 != qlen { bail!("len and qlen are inconsistent: len = {len}, qlen = {qlen}"); }

    str = &str[..len.try_into()?];

    // clip zeros off end
    while let &[ref rest @ .., 0] = str { str = rest }
    
    Ok(str)
}

#[derive(Clone, Copy, Debug)]
struct DecodeUnimplemented;

impl std::error::Error for DecodeUnimplemented {}

impl std::fmt::Display for DecodeUnimplemented {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("decode unimplemented")
    }
}

impl Action {
    const OP_SPEAKER: u32 = 0xd4;
    const OP_LINE: u32 = 0xd2;
    const OP_YIELD: u32 = 0xd3;
    const OP_CHOICE: u32 = 0xe7;
    
    fn op(&self) -> anyhow::Result<Operation<'_>> {
        match *self {
            Action { call: true, .. } => Ok(Operation::Raw(self)),
            Action { opcode: Self::OP_SPEAKER, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr)] = &params[..] else { bail!("bad speaker: params = {params:08X?}"); };
                Ok(Operation::Speaker(self.addr, decode_string(&data[addr as usize..])?))
            }
            Action { opcode: Self::OP_LINE, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr)] = &params[..] else { bail!("bad line: params = {params:08X?}"); };
                Ok(Operation::Line(self.addr, decode_string(&data[addr as usize..])?))
            },
            Action { opcode: Self::OP_CHOICE, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr), Parameter::Value(i)] = &params[..] else { bail!("bad choice: params = {params:08X?}"); };
                Ok(Operation::Choice(self.addr, i, decode_string(&data[addr as usize..])?))
            },
            Action { opcode: Self::OP_YIELD, ref params, ref data, .. } => {
                ensure!(params.is_empty() && data.is_empty(), "bad line end");
                Ok(Operation::Yield(self.addr))
            }
            _ => Ok(Operation::Raw(self))
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut db = Connection::open(args.db)?;

    let mut tx = db.transaction()?;
    tx.set_drop_behavior(DropBehavior::Commit);

    tx.execute("CREATE TABLE IF NOT EXISTS lines(
        scriptid INTEGER REFERENCES script (id),
        address INTEGER,
        session TEXT,
        speaker TEXT NOT NULL,
        line TEXT NOT NULL,
        PRIMARY KEY (scriptid, address, session)
    ) WITHOUT ROWID, STRICT", ())?;

    let mut file = BufReader::with_capacity(0x2000, tx.blob_open(DatabaseName::Main, "script", "data", args.script_id.into(), true)?);

    if !file.fill_buf()?.starts_with(STCM2_MAGIC) {
        bail!("bad magic");
    }
    file.consume(STCM2_MAGIC.len());

    let export_addr = file.read_u32::<LE>()? as u64;

    file.seek(SeekFrom::Start(export_addr))?;

    let mut exports = Vec::new();

    let mut buffer = [0; 32];
    loop {
        ensure!(file.read_u32::<LE>()? == 0, "export does not begin with 0");
        match file.read_exact(&mut buffer) {
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => break,
            x => x
        }?;
        exports.push(Export { name: buffer, addr: file.read_u32::<LE>()?.into() });
    }

    file.rewind()?;

    // CODE_START_ will be 16-byte aligned
    while !file.fill_buf()?.starts_with(CODE_START_MAGIC) {
        file.consume(16);
    }
    file.consume(CODE_START_MAGIC.len());

    let mut actions = Vec::new();
    loop {
        let addr = file.stream_position()? as u32;
        
        let global_call = file.read_u32::<LE>()?;
        let opcode = file.read_u32::<LE>()?;
        let nparams = file.read_u32::<LE>()?;
        let length = file.read_u32::<LE>()?;

        if length == 0 { break; }

        let call = match global_call {
            0 => false,
            1 => true,
            v => bail!("global_call = {v:08X}")
        };
        let mut params = Vec::with_capacity(nparams as usize);
        for _ in 0..nparams {
            let mut buffer = [0; 3];
            file.read_u32_into::<LE>(&mut buffer)?;
            params.push(Parameter::parse(buffer, addr + 16 + 12*nparams, length - 16 - 12*nparams)?);
        }

        let mut data = Vec::new();
        let ndata = length - 16 - 12*nparams;
        if ndata > 0 {
            data.resize(ndata as usize, 0);
            file.read_exact(&mut data)?;
        }

        let export = exports.iter().position(|e| e.addr == u64::from(addr)).map(|i| exports.swap_remove(i).name);

        actions.push(Action { addr, export, call, opcode, params, data });
    }
    file.into_inner().close()?;

    ensure!(exports.is_empty(), "exports left over!");

    let parsed = parse::parse(actions.iter().filter_map(|act| act.op().ok()));

    let speakers = HashMap::<Option<&str>, &str>::from(dict::SPEAKERS);

    let mut n = 0;
    for d in parsed {
        if let parse::Dialogue::Line { addr, speaker, line } = d {
            tx.execute("INSERT OR REPLACE INTO lines VALUES(?, ?, 'original', ?, ?)", (args.script_id, addr, speakers.get(&speaker.as_deref()).unwrap(), &line))?;
        } else if let parse::Dialogue::Choice { .. } = d {
            n += 1;
        }
    }
    tx.commit()?;

    println!("found {n} choices");

    Ok(())
}
