use std::{cmp::Ordering, fs::File, io::{self, Seek, SeekFrom, Write}};
use anyhow::ensure;
use rusqlite::Connection;

use super::{Args, Entry, SECTOR_SIZE, UNI2_MAGIC};

fn write_u32_le(mut w: impl Write, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

static ZEROS: [u8; SECTOR_SIZE as usize] = [0; SECTOR_SIZE as usize];

const TABLE_SECT: u32 = 1;
const DATA_SECT: u32 = 2;

fn to_sector(mut w: impl Write + Seek, sect: u64) -> anyhow::Result<()> {
    let sectpos = sect*SECTOR_SIZE;
    let endpos = w.seek(SeekFrom::End(0))?;
    match sectpos.cmp(&endpos) {
        Ordering::Less => {
            w.seek(SeekFrom::Current(i64::try_from(sectpos)? - i64::try_from(endpos)?))?;
        },
        Ordering::Equal => (),
        Ordering::Greater => {
            let mut remaining = usize::try_from(sectpos - endpos)?;
            while remaining > 0 {
                remaining -= match w.write(&ZEROS[..remaining.min(ZEROS.len())]) {
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    r => r?
                };
            }
        }
    }
    Ok(())
}

fn next_sector(mut w: impl Write + Seek) -> anyhow::Result<u64> {
    let pos = w.stream_position()?;
    let sect = pos.div_ceil(SECTOR_SIZE);
    w.write_all(&ZEROS[..usize::try_from(sect*SECTOR_SIZE - pos)?])?;
    Ok(sect)
}

pub fn build(db: Connection, args: Args) -> anyhow::Result<()> {
    let mut entries = Vec::new();

    let mut file = File::create(args.uni)?;

    file.write_all(UNI2_MAGIC)?;
    let len = db.query_row("SELECT COUNT(*) FROM scripts", (), |row| row.get::<_, usize>(0))?;
    ensure!(u64::try_from(len)? <= SECTOR_SIZE / 16);
    write_u32_le(&mut file, len.try_into()?)?;
    write_u32_le(&mut file, TABLE_SECT)?;
    write_u32_le(&mut file, DATA_SECT)?;
    
    to_sector(&mut file, DATA_SECT.into())?;

    let mut stmt = db.prepare("
        SELECT id, IFNULL(ps.script, s.script) FROM scripts AS s LEFT JOIN patchedscripts AS ps USING (id)
    ")?;
    // let mut stmt = tx.prepare("
    //     SELECT id, script FROM scripts
    // ")?;
    let mut rows = stmt.query(())?;

    let mut current_sect = next_sector(&mut file)?;
    
    while let Some(row) = rows.next()? {
        let start_sect = current_sect;
        let id = row.get_ref(0)?.as_i64()?;
        let script = row.get_ref(1)?.as_blob()?;
        file.write_all(script)?;
        current_sect = next_sector(&mut file)?;
        entries.push(Entry { id: id.try_into()?, start_sect: start_sect - u64::from(DATA_SECT), size_sect: current_sect - start_sect, size: script.len().try_into()? });
    }

    to_sector(&mut file, TABLE_SECT.into())?;
    for Entry { id, start_sect, size_sect, size } in entries {
        write_u32_le(&mut file, id)?;
        write_u32_le(&mut file, start_sect.try_into()?)?;
        write_u32_le(&mut file, size_sect.try_into()?)?;
        write_u32_le(&mut file, size.try_into()?)?;
    }

    {
        let pos = file.stream_position()?;
        ensure!(pos >= u64::from(TABLE_SECT)*SECTOR_SIZE && pos < u64::from(DATA_SECT)*SECTOR_SIZE);
    }

    Ok(())
}