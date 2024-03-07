mod biendian;

use std::io::{self, Read};

use biendian::BiEndian as _;
use bytes::{Bytes, BytesMut, Buf, BufMut as _};

const SECTOR_SIZE: usize = 0x800;

fn yank<const N: usize>(mut r: impl Buf) -> [u8; N] {
    let mut buf = [0; N];
    r.copy_to_slice(&mut buf);
    buf
}

#[derive(Debug, Clone)]
struct PathEntry {
    lba: u32,
    parent: u16,
    ident: Bytes
}

impl PathEntry {
    fn to_bytes(&self, le: bool) -> Bytes {
        let mut b = BytesMut::new();
        b.put_u8(self.ident.len().try_into().unwrap());
        b.put_u8(0);
        if le { b.put_u32_le(self.lba) } else { b.put_u32(self.lba) }

    }
}

#[derive(Debug, Clone)]
struct Directory {
    lba: u32,
    size: u32,
    date: [u8; 7],
    flags: u8,
    set_idx: u16,
    ident: Bytes
}

impl Directory {
    fn from_bytes(mut b: impl Buf) -> Self {
        let _len = b.get_u8();
        assert_eq!(0, b.get_u8(), "ear");
        let lba = u32::from_bi_endian(yank(&mut b)).unwrap();
        let size = u32::from_bi_endian(yank(&mut b)).unwrap();
        let date = yank(&mut b);
        let flags = b.get_u8();
        let set_idx = u16::from_bi_endian(yank(&mut b)).unwrap();
        let ident_len = b.get_u8();
        let ident = b.copy_to_bytes(ident_len.into());

        Self {
            lba,
            size,
            date,
            flags,
            set_idx,
            ident
        }
    }

    fn to_bytes(&self) -> Bytes {
        let mut b = BytesMut::new();
        b.put_u8(0);
        b.put_u8(0);
        b.put_slice(&self.lba.to_bi_endian());
        b.put_slice(&self.size.to_bi_endian());
        b.put_slice(&self.date);
        b.put_u8(self.flags);
        b.put_u8(0);
        b.put_u8(0);
        b.put_slice(&self.set_idx.to_bi_endian());
        b.put_u8(self.ident.len().try_into().unwrap());
        b.put_slice(&self.ident);
        b[0] = b.len().try_into().unwrap();
        if b.len() % 2 != 0 {
            b.put_u8(0);
        }
        b.freeze()
    }
}

#[derive(Debug, Clone)]
struct Pvd {
    preamble: [u8; 32768],
    system_ident: [u8; 32],
    volume_ident: [u8; 32],
    volume_lbs: u32,
    set_disks: u16,
    set_idx: u16,

}

impl Pvd {
    const BEGIN: [u8; 80] = *b"\x01CD001\x01\0";
}