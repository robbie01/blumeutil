use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, bail, ensure, Context as _};
use bytes::{Buf as _, BufMut as _, Bytes, BytesMut};

//const STCM2_MAGIC: &[u8] = b"STCM2 File Make By Minku 07.0\0\0\0";
const STCM2_MAGIC: &[u8] = b"STCM2";
const STCM2_TAG_LENGTH: usize = 32 - STCM2_MAGIC.len();
const GLOBAL_DATA_MAGIC: &[u8] = b"GLOBAL_DATA\0\0\0\0\0";
const GLOBAL_DATA_OFFSET: usize = STCM2_MAGIC.len() + STCM2_TAG_LENGTH + 12*4 + GLOBAL_DATA_MAGIC.len();
const CODE_START_MAGIC: &[u8] = b"CODE_START_\0";
const EXPORT_DATA_MAGIC: &[u8] = b"EXPORT_DATA\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address {
    pub orig: u32,
    pub sub: u32
}

#[derive(Clone)]
#[allow(dead_code)]
pub enum Operation {
    Speaker {
        addr: u32,
        s: Bytes
    },
    Line {
        addr: u32,
        s: Bytes
    },
    Choice {
        addr: u32,
        id: u32,
        s: Bytes
    },
    Unknown(Action)
}

#[derive(Clone, Copy, Debug)]
pub enum Parameter {
    GlobalPointer(u32),
    LocalPointer(u32),
    Value(u32)
}

impl Parameter {
    fn parse(value: [u32; 3], data_addr: u32, data_len: u32) -> anyhow::Result<Self> {
        match value {
            [0xffffff41, addr, 0xff000000] => Ok(Self::GlobalPointer(addr)),
            [addr, 0xff000000, 0xff000000] if (addr & 0xff000000 != 0xff000000) && addr >= data_addr && addr < data_addr+data_len => Ok(Self::LocalPointer(addr-data_addr)),
            [value, 0xff000000, 0xff000000] => Ok(Self::Value(value)),
            _ => Err(anyhow!("bad parameter: {value:08X?}"))
        }
    }

    fn to_u32s(self, resolvers: &mut Vec<Resolver>, old_act_addr: Address, current_pos: usize) -> [u32; 3] {
        match self {
            Self::GlobalPointer(addr) => {
                let canary = rand::random();
                resolvers.push(Box::new(move |refs, buf| {
                    assert_eq!(canary, u32::from_le_bytes(buf[current_pos+4..current_pos+8].try_into().unwrap()));
                    let Some(&dest) = refs.get(&Reference::Action(Address { orig: addr, sub: 0 })) else { return false };
                    buf[current_pos+4..current_pos+8].copy_from_slice(&dest.to_le_bytes());
                    true
                }));
                [0xffffff41, canary, 0xff000000]
            },
            Self::LocalPointer(addr) => {
                let canary = rand::random();
                resolvers.push(Box::new(move |refs, buf| {
                    assert_eq!(canary, u32::from_le_bytes(buf[current_pos..current_pos+4].try_into().unwrap()));
                    let Some(&dest) = refs.get(&Reference::ActionData(old_act_addr)) else { return false };
                    buf[current_pos..current_pos+4].copy_from_slice(&(dest + addr).to_le_bytes());
                    true
                }));
                [canary, 0xff000000, 0xff000000]
            },
            Self::Value(value) => [value, 0xff000000, 0xff000000]
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Action {
    pub export: Option<Bytes>,
    pub call: bool,
    pub opcode: u32,
    pub params: Vec<Parameter>,
    pub data: Bytes
}

pub fn decode_string(addr: u32, mut str: Bytes) -> anyhow::Result<Bytes> {
    let init = str.split_to(addr as usize);

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

    ensure!(init.is_empty() && tail.is_empty());
    
    Ok(str)
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct DecodeUnimplemented;

impl std::error::Error for DecodeUnimplemented {}

impl std::fmt::Display for DecodeUnimplemented {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("decode unimplemented")
    }
}

impl Action {
    pub const OP_SPEAKER: u32 = 0xd4;
    pub const OP_YIELD: u32 = 0xd3;
    pub const OP_LINE: u32 = 0xd2;
    pub const OP_CHOICE: u32 = 0xe7;
    
    pub fn op(self, orig_addr: u32) -> anyhow::Result<Operation> {
        match self {
            Action { call: true, .. } => Ok(Operation::Unknown(self)),
            Action { opcode: Self::OP_SPEAKER, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr)] = &params[..] else { bail!("bad speaker: params = {params:08X?}"); };
                Ok(Operation::Speaker { addr: orig_addr, s: decode_string(addr, data.clone())? })
            }
            Action { opcode: Self::OP_LINE, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr)] = &params[..] else { bail!("bad line: params = {params:08X?}"); };
                Ok(Operation::Line { addr: orig_addr, s: decode_string(addr, data.clone())? })
            },
            Action { opcode: Self::OP_CHOICE, ref params, ref data, .. } => {
                let &[Parameter::LocalPointer(addr), Parameter::Value(id)] = &params[..] else { bail!("bad choice: params = {params:08X?}"); };
                Ok(Operation::Choice { addr: orig_addr, id, s: decode_string(addr, data.clone())? })
            },
            _ => Ok(Operation::Unknown(self))
        }
    }

    fn to_bytes(&self, addr: Address, resolvers: &mut Vec<Resolver>, out: &mut BytesMut) -> anyhow::Result<()> {
        let new_addr = out.len();
        let canary = rand::random();

        let opcode = self.opcode;
        if self.call {
            resolvers.push(Box::new(move |refs, buf| {
                assert_eq!(canary, u32::from_le_bytes(buf[new_addr+4..new_addr+8].try_into().unwrap()));
                let Some(&dest) = refs.get(&Reference::Action(Address { orig: opcode, sub: 0 })) else { return false };
                buf[new_addr+4..new_addr+8].copy_from_slice(&dest.to_le_bytes());
                true
            }));
        }

        let nparams = self.params.len().try_into()?;
        let ndata: u32 = self.data.len().try_into()?;
        let length = ndata + 16 + 12*nparams;
        out.put_u32_le(self.call.into());
        out.put_u32_le(if self.call { canary } else { opcode });
        out.put_u32_le(nparams);
        out.put_u32_le(length);
        for param in self.params.iter() {
            for x in param.to_u32s(resolvers, addr, out.len()) {
                out.put_u32_le(x);
            }
        }
        out.put_slice(&self.data);

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Reference {
    Action(Address),
    ActionData(Address)
}

#[derive(Clone, Debug)]
pub struct Stcm2 {
    pub tag: Bytes,
    pub global_data: Bytes,
    pub actions: BTreeMap<Address, Action>
}

pub fn from_bytes(mut file: Bytes) -> anyhow::Result<Stcm2> {
    let start_addr = file.as_ptr();
    let get_pos = |file: &Bytes| file.as_ptr() as usize - start_addr as usize;

    ensure!(file.starts_with(STCM2_MAGIC));
    file.advance(STCM2_MAGIC.len());
    let tag = file.split_to(STCM2_TAG_LENGTH);
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

        let res = actions.insert(Address { orig: addr, sub: 0 }, Action { export: None, call, opcode, params, data });
        ensure!(res.is_none());
    }

    ensure!(file.starts_with(EXPORT_DATA_MAGIC));
    file.advance(EXPORT_DATA_MAGIC.len());

    for _ in 0..export_len {
        ensure!(file.get_u32_le() == 0);
        let export = file.split_to(32);
        let addr = file.get_u32_le();
        let act = actions.get_mut(&Address { orig: addr, sub: 0 }).context("export does not match known action")?;
        ensure!(act.export.is_none());
        act.export = Some(export);
    }

    Ok(Stcm2 {
        tag,
        global_data,
        actions
    })
}

type Resolver = Box<dyn Fn(&mut HashMap<Reference, u32>, &mut [u8]) -> bool>;

pub fn to_bytes(input: Stcm2) -> anyhow::Result<BytesMut> {
    let mut resolvers = Vec::<Resolver>::new();
    let mut output = BytesMut::from(STCM2_MAGIC);
    output.put_slice(&input.tag);
    let mut refs = HashMap::<Reference, u32>::default();
    let export_addr_loc = output.len();
    output.put_u32_le(0);
    output.put_u32_le(input.actions.iter().filter_map(|(_, act)| act.export.as_ref()).count().try_into()?);

    for _ in 0..10 {
        output.put_u32_le(0);
    }

    output.put_slice(GLOBAL_DATA_MAGIC);
    output.put_slice(&input.global_data);
    output.put_slice(CODE_START_MAGIC);

    for (&addr, act) in input.actions.iter() {
        refs.insert(Reference::Action(addr), output.len().try_into()?);
        refs.insert(Reference::ActionData(addr), (output.len() + 16 + 12*act.params.len()).try_into()?);
        act.to_bytes(addr, &mut resolvers, &mut output)?;
    }

    output.put_slice(EXPORT_DATA_MAGIC);
    let export_addr: u32 = output.len().try_into()?;
    output[export_addr_loc..export_addr_loc+4].copy_from_slice(&export_addr.to_le_bytes());
    for (addr, act) in input.actions {
        if let Some(export) = act.export {
            output.put_u32_le(0);
            output.put_slice(&export);
            output.put_u32_le(*refs.get(&Reference::Action(addr)).unwrap());
        }
    }
    while output.len() % 16 != 0 {
        output.put_u32_le(0);
    }

    let mut last_len = None;
    while !resolvers.is_empty() {
        resolvers.retain(|r| !r(&mut refs, &mut output));
        ensure!(Some(resolvers.len()) != last_len, "made no progress");
        last_len = Some(resolvers.len());
    }

    Ok(output)
}
