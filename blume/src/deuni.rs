use std::{path::PathBuf, fs::File, iter, io::{self, BufReader, SeekFrom, Read as _, Seek as _, BufRead as _}};
use anyhow::bail;
use clap::Parser;
use byteorder::{LittleEndian as LE, ReadBytesExt as _};
use rusqlite::{blob::ZeroBlob, Connection, DatabaseName, DropBehavior};

const UNI2_MAGIC: &[u8] = b"UNI2\0\0\x01\0";
const SECTOR_SIZE: u64 = 0x800;

#[derive(Parser)]
pub struct Args {
    #[arg(help = "Path to the uni file")]
    uni: PathBuf
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    id: u32,
    start_sect: u64,
    size_sect: u64,
    size: u64
}

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

pub fn run(mut db: Connection, args: Args) -> anyhow::Result<()> {
    let mut file = BufReader::with_capacity(SECTOR_SIZE as usize, File::open(args.uni)?);

    if !file.fill_buf()?.starts_with(UNI2_MAGIC) {
        bail!("bad magic");
    }
    file.consume(UNI2_MAGIC.len());

    let n = file.read_u32::<LE>()? as usize;
    let table_sect = file.read_u32::<LE>()? as u64;
    let data_sect = file.read_u32::<LE>()? as u64;

    file.seek(SeekFrom::Start(table_sect*SECTOR_SIZE))?;

    let entries = iter::repeat_with(|| {
        let mut buffer = [0; 4];
        file.read_u32_into::<LE>(&mut buffer)?;
        let [id, start_sect, size_sect, size] = buffer;
        Ok(Entry { id, start_sect: start_sect.into(), size_sect: size_sect.into(), size: size.into() })
    }).take(n).collect::<anyhow::Result<Vec<Entry>>>()?;

    if !validate(&entries) {
        bail!("table failed validation");
    }

    println!("found {n} entries");

    let mut tx = db.transaction()?;
    tx.set_drop_behavior(DropBehavior::Commit);

    for Entry { id, start_sect, size, .. } in entries {
        let sp = tx.savepoint()?;
        sp.execute("INSERT INTO scripts VALUES(?, ?)", (id, ZeroBlob(size.try_into()?)))?;

        let mut blob = sp.blob_open(DatabaseName::Main, "scripts", "script", id.into(), false)?;

        file.seek(SeekFrom::Start((data_sect+start_sect)*SECTOR_SIZE))?;

        if size != io::copy(&mut file.by_ref().take(size), &mut blob)? {
            bail!("EOF reached while copying {id:X}");
        }

        blob.close()?;
        sp.commit()?;

        println!("copied {id:X}; {size} bytes");
    }

    tx.commit()?;

    Ok(())
}
