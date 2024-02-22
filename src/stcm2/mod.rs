mod parse;

use std::io;
use anyhow::{bail, ensure, anyhow};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use clap::Parser;
use encoding_rs::SHIFT_JIS;
use rusqlite::{Connection, DatabaseName, DropBehavior};

const STCM2_MAGIC: &[u8] = b"STCM2 File Make By Minku 07.0\0\0\0";
const CODE_START_MAGIC: &[u8] = b"CODE_START_\0";

#[derive(Parser)]
#[group(required = true, multiple = false)]
pub struct Args {
    #[arg(short, help = "analyze all scripts", group = "script")]
    all: bool,
    #[arg(help = "ids of scripts to analyze", group = "script")]
    id: Vec<u32>,
    #[arg(from_global)]
    dry_run: bool
}

#[derive(Clone)]
pub enum Operation {
    Speaker(u32, Bytes),
    Line(u32, Bytes),
    Choice(u32, u32, Bytes),
    Yield(u32),
    Unknown(Action)
}

impl std::fmt::Debug for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Operation::*;

        match *self {
            Yield(ref addr) => f.debug_tuple("Yield").field(addr).finish(),
            Speaker(ref addr, ref s) => f.debug_tuple("Speaker").field(addr).field(&SHIFT_JIS.decode_without_bom_handling(s).0).finish(),
            Line(ref addr, ref s) => f.debug_tuple("Line").field(addr).field(&SHIFT_JIS.decode_without_bom_handling(s).0).finish(),
            Choice(ref addr, ref i, ref s) => f.debug_tuple("Choice").field(addr).field(i).field(&SHIFT_JIS.decode_without_bom_handling(s).0).finish(),
            Unknown(ref act) => f.debug_tuple("Unknown").field(act).finish()
        }
    }
}

#[derive(Clone, Debug)]
struct Export {
    name: Bytes,
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
    export: Option<Bytes>,
    call: bool,
    opcode: u32,
    params: Vec<Parameter>,
    data: Bytes
}

fn decode_string(addr: u32, mut str: Bytes) -> anyhow::Result<(Bytes, Bytes, Bytes)> {
    let init = str.split_to(addr as usize);

    if str.get_u32_le() != 0 { bail!("string magic isn't 0"); }
    let qlen = str.get_u32_le();
    if str.get_u32_le() != 1 { bail!("string magic isn't 1"); }
    let len = str.get_u32_le();
    if len/4 != qlen { bail!("len and qlen are inconsistent: len = {len}, qlen = {qlen}"); }

    let tail = str.split_off(len.try_into()?);

    // clip zeros off end
    while let Some(0) = str.last() {
        str.truncate(str.len()-1);
    }
    
    Ok((init, str, tail))
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
    
    fn op(self) -> anyhow::Result<Operation> {
        match self {
            Action { call: true, .. } => Ok(Operation::Unknown(self)),
            Action { opcode: Self::OP_SPEAKER, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr)] = &params[..] else { bail!("bad speaker: params = {params:08X?}"); };
                Ok(Operation::Speaker(self.addr, decode_string(addr, data.clone())?.1))
            }
            Action { opcode: Self::OP_LINE, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr)] = &params[..] else { bail!("bad line: params = {params:08X?}"); };
                Ok(Operation::Line(self.addr, decode_string(addr, data.clone())?.1))
            },
            Action { opcode: Self::OP_CHOICE, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr), Parameter::Value(i)] = &params[..] else { bail!("bad choice: params = {params:08X?}"); };
                Ok(Operation::Choice(self.addr, i, decode_string(addr, data.clone())?.1))
            },
            Action { opcode: Self::OP_YIELD, ref params, ref data, .. } => {
                ensure!(params.is_empty() && data.is_empty(), "bad line end");
                Ok(Operation::Yield(self.addr))
            }
            _ => Ok(Operation::Unknown(self))
        }
    }
}

pub fn run(mut db: Connection, args: Args) -> anyhow::Result<()> {
    let mut tx = db.transaction()?;
    tx.set_drop_behavior(DropBehavior::Commit);

    let scripts = if args.all {
        let mut stmt = tx.prepare("SELECT id, LENGTH(script) FROM scripts WHERE id >= 100 AND id < 200")?;
        let scripts = stmt.query_map((), |row| <(u32, usize)>::try_from(row))?.collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        scripts
    } else {
        let mut stmt = tx.prepare(&format!(
            "SELECT id, LENGTH(script) FROM scripts WHERE id IN ({})",
            vec!["?"; args.id.len()].join(", ")
        ))?;
        let scripts = stmt.query_map(rusqlite::params_from_iter(args.id), |row| <(u32, usize)>::try_from(row))?.collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        scripts
    };

    let mut stmt = tx.prepare("INSERT OR IGNORE INTO lines(scriptid, address, speaker, line) VALUES (?, ?, ?, ?)")?;

    for (script_id, script_size) in scripts {
        println!("Processing script {script_id}");
        let mut file = BytesMut::with_capacity(script_size).writer();
        io::copy(
            &mut tx.blob_open(DatabaseName::Main, "scripts", "script", script_id.into(), true)?,
            &mut file
        )?;
        let file = file.into_inner().freeze();

        let export_addr = {
            let mut file = file.clone();
            if !file.starts_with(STCM2_MAGIC) {
                bail!("bad magic");
            }
            file.advance(STCM2_MAGIC.len());
            file.get_u32_le() as usize
        };

        let mut exports = {
            let mut file = file.clone();
            file.advance(export_addr);

            let mut exports = Vec::new();

            loop {
                if file.len() < 40 { break }
                ensure!(file.get_u32_le() == 0, "export does not begin with 0");
                exports.push(Export { name: file.split_to(32), addr: file.get_u32_le().into() });
            }

            exports
        };

        let mut file = file;
        let start_addr = file.as_ptr();

        // CODE_START_ will be 16-byte aligned
        while !file.starts_with(CODE_START_MAGIC) {
            file.advance(16);
        }
        file.advance(CODE_START_MAGIC.len());

        let mut actions = Vec::new();
        loop {
            let addr = unsafe { file.as_ptr().offset_from(start_addr) }.try_into()?;
            
            let global_call = file.get_u32_le();
            let opcode = file.get_u32_le();
            let nparams = file.get_u32_le();
            let length = file.get_u32_le();

            if length == 0 { break }

            let call = match global_call {
                0 => false,
                1 => true,
                v => bail!("global_call = {v:08X}")
            };
            let mut params = Vec::with_capacity(nparams as usize);
            for _ in 0..nparams {
                let buffer = [file.get_u32_le(), file.get_u32_le(), file.get_u32_le()];
                params.push(Parameter::parse(buffer, addr + 16 + 12*nparams, length - 16 - 12*nparams)?);
            }

            let ndata = length - 16 - 12*nparams;
            let data = file.split_to(ndata as usize);

            let export = exports.iter()
                .position(|e| e.addr == u64::from(addr))
                .map(|i| exports.swap_remove(i).name);

            actions.push(Action { addr, export, call, opcode, params, data });
        }

        ensure!(exports.is_empty(), "exports left over!");

        let parsed = parse::parse(actions.into_iter().filter_map(|act| act.op().ok()))?;

        let mut n = 0;
        for d in parsed {
            if let parse::Dialogue::Line { addr, speaker, line } = d {
                stmt.execute((script_id, addr, speaker, line))?;
            } else if let parse::Dialogue::Choice { .. } = d {
                n += 1;
            }
        }
        println!("found {n} choices");
    }

    drop(stmt);

    if args.dry_run {
        tx.rollback()?;
    } else {
        tx.commit()?;
    }

    Ok(())
}
