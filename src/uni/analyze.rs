use std::{fs::File, io::{self, BufReader, SeekFrom, Read as _, Seek as _, BufRead as _}};
use anyhow::ensure;
use rusqlite::{blob::ZeroBlob, Connection, DatabaseName};
use super::{Args, Entry, UNI2_MAGIC, SECTOR_SIZE};

fn validate(mut entries: &[Entry]) -> bool {
    while !entries.is_empty() {
        let x = entries[0];
        entries = &entries[1..];

        // ensure size matches sector size
        if !(x.size > (x.size_sect-1)*SECTOR_SIZE && x.size <= x.size_sect*SECTOR_SIZE) { return false; }

        // ensure ids and regions are strictly ascending (implies unique and nonoverlapping)
        if entries.first().is_some_and(|&y| x.id >= y.id || x.start_sect >= y.start_sect || y.start_sect < x.start_sect+x.size_sect) { return false; }
    }
    true
}

fn read_u32_le(mut r: impl io::Read) -> io::Result<u32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn analyze(mut db: Connection, args: Args) -> anyhow::Result<()> {
    let mut file = BufReader::with_capacity(SECTOR_SIZE as usize, File::open(args.uni)?);

    ensure!(file.fill_buf()?.starts_with(UNI2_MAGIC), "bad magic");
    file.consume(UNI2_MAGIC.len());

    let n = read_u32_le(&mut file)? as usize;
    let table_sect = read_u32_le(&mut file)? as u64;
    let data_sect = read_u32_le(&mut file)? as u64;

    file.seek(SeekFrom::Start(table_sect*SECTOR_SIZE))?;

    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let id = read_u32_le(&mut file)?;
        let start_sect = read_u32_le(&mut file)?;
        let size_sect = read_u32_le(&mut file)?;
        let size = read_u32_le(&mut file)?;
        entries.push(Entry { id, start_sect: start_sect.into(), size_sect: size_sect.into(), size: size.into() })
    }

    ensure!(validate(&entries), "table failed validation");

    println!("found {n} entries");

    let tx = db.transaction()?;
    let mut stmt = tx.prepare("INSERT INTO scripts(id, script) VALUES(?, ?)")?;

    for Entry { id, start_sect, size, .. } in entries {
        stmt.execute((id, ZeroBlob(size.try_into()?)))?;

        let mut blob = tx.blob_open(DatabaseName::Main, "scripts", "script", id.into(), false)?;

        file.seek(SeekFrom::Start((data_sect+start_sect)*SECTOR_SIZE))?;

        ensure!(size == io::copy(&mut file.by_ref().take(size), &mut blob)?, "EOF reached while copying {id:X}");

        blob.close()?;
    }

    drop(stmt);
    tx.commit()?;

    Ok(())
}