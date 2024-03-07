use std::{fs::File, io, path::PathBuf};

use anyhow::{ensure, Context};
use bytes::{Buf, BufMut as _, BytesMut};
use clap::Parser;
use encoding_rs::SHIFT_JIS;

mod uni;

static ART2_MAGIC: &[u8; 4] = b"ART2";

#[derive(Parser)]
struct Args {
    uni: PathBuf
}

#[inline(always)]
fn transform_palette_id(id: u8) -> u8 {
    // found using a PCSX2 texture dump, imageio, numpy, and a *lot* of guesswork
    if id.checked_sub(8).is_some_and(|z| z % 32 < 16) {
        id ^ 24
    } else {
        id
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    rayon::scope(|scope| {
        uni::analyze(args.uni, |id, f| {
            let mut img = BytesMut::new().writer();
            io::copy(f, &mut img)?;
            let mut img = img.into_inner().freeze();

            scope.spawn(move |_scope| (|| {
                if !img.starts_with(ART2_MAGIC) {
                    return Ok(());
                }

                ensure!(img.starts_with(ART2_MAGIC));
                img.advance(ART2_MAGIC.len());
                let bpp = img.get_u32_le();
                ensure!(bpp == 8);
                let width = img.get_u32_le();
                let height = img.get_u32_le();

                let name = img.split_to(16);
                let name = SHIFT_JIS.decode_without_bom_handling_and_without_replacement(&name).unwrap_or_else(|| format!("{id:08X}").into());
                let filename = format!("{}.png", name.trim_end_matches('\0'));

                println!("dumping {filename}");

                let palette = img.split_off((width as usize) * (height as usize) * ((bpp / 8) as usize));
                ensure!(palette.len() == 4 << bpp);

                let mut plte = vec![0u8; 3 << bpp];
                let mut trns = vec![0u8; 1 << bpp];

                for (id, c) in palette.chunks_exact(4).enumerate() {
                    let [r, g, b, a] = c.try_into()?;
                    let id = usize::from(transform_palette_id(id.try_into()?));
                    plte[id*3..id*3+3].copy_from_slice(&[r, g, b]);
                    trns[id] = a;
                }

                let mut out = png::Encoder::new(File::create(filename)?, width, height);
                out.set_color(png::ColorType::Indexed);
                out.set_filter(png::FilterType::Paeth);
                out.set_depth(png::BitDepth::from_u8(bpp.try_into()?).context("bruh")?);
                out.set_palette(plte);
                out.set_trns(trns);
                let mut out = out.write_header()?;
                out.write_image_data(&img)?;
                out.finish()?;

                Ok(())
            })().unwrap());

            Ok(())
        })
    })
}