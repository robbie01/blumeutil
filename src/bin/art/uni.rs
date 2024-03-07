use std::{fs::File, io::{self, BufRead as _, BufReader, Read, Seek as _, SeekFrom}, path::Path};
use anyhow::ensure;

const UNI2_MAGIC: &[u8] = b"UNI2\0\0\x01\0";
const SECTOR_SIZE: u64 = 0x800;

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

fn read_u32_le(mut r: impl io::Read) -> io::Result<u32> {
    let mut buf = [0; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

pub fn analyze(uni: impl AsRef<Path>, mut cb: impl FnMut(u32, &mut dyn Read) -> anyhow::Result<()>) -> anyhow::Result<()> {
    let mut file = BufReader::with_capacity(SECTOR_SIZE as usize, File::open(uni)?);

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

    for Entry { id, start_sect, size, .. } in entries {
        file.seek(SeekFrom::Start((data_sect+start_sect)*SECTOR_SIZE))?;
        cb(id, &mut file.by_ref().take(size))?;
    }

    Ok(())
}