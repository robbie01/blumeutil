use std::collections::{BTreeMap, HashMap};

use anyhow::{bail, ensure};
use bytes::{BufMut as _, Bytes, BytesMut};
use encoding_rs::SHIFT_JIS;
use rusqlite::Connection;
use super::{format::{self, Action, Address, Parameter, Stcm2}, Args};

const MAX_LINE_LENGTH: usize = 44; // game will print a debug message if the line is over 45 halfwidth chars incl newline

fn encode_string(s: &str) -> anyhow::Result<BytesMut> {
    let (enc, _, false) = SHIFT_JIS.encode(s) else { bail!("unmappable character! what is you doing???") };
    let mut b = BytesMut::new();
    b.put_u32_le(0);
    b.put_u32_le(enc.len().div_ceil(4).try_into()?);
    b.put_u32_le(1);
    b.put_u32_le(enc.len().try_into()?);
    b.put_slice(&enc);
    while b.len() % 4 != 0 {
        b.put_u8(0);
    }

    Ok(b)
}

fn increment_address(addr: Address) -> Address {
    match addr {
        Address::Original(x) => Address::User(x, 0),
        Address::User(x, y) => Address::User(x, y+1)
    }
}

fn charwidth(c: char) -> usize {
    if c.is_ascii() { 1 } else { 2 }
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
                new_actions.insert(addr, act);
                cur_addr = Some(increment_address(addr));
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
                        if let Some(translation) = tls.remove(&addr.orig()) {
                            buf_actions.clear();
                            let mut translation = translation.chars().peekable();
                            while translation.peek().is_some() {
                                let mut s = String::new();
                                let mut count = 0;
                                while let Some(&c) = translation.peek() {
                                    if count + charwidth(c) > MAX_LINE_LENGTH { break }
                                    translation.next(); // consume
                                    s.push(c);
                                    count += charwidth(c);
                                }
                                new_actions.insert(addr, Action {
                                    export: None,
                                    call: false,
                                    opcode: Action::OP_LINE,
                                    params: vec![Parameter::LocalPointer(0)],
                                    data: encode_string(&s)?.freeze()
                                });
                                addr = increment_address(addr);
                            }
                        } else {
                            new_actions.append(&mut buf_actions);
                        }
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
