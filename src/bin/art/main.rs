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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    rayon::scope(|scope| {
        uni::analyze(args.uni, |id, f| {
            let mut b = BytesMut::new().writer();
            io::copy(f, &mut b)?;
            let mut b = b.into_inner();

            scope.spawn(move |_scope| (|| {
                if !b.starts_with(ART2_MAGIC) {
                    return Ok(());
                }

                ensure!(b.starts_with(ART2_MAGIC));
                b.advance(ART2_MAGIC.len());
                let bpp = b.get_u32_le();
                ensure!(bpp == 8);
                let width = b.get_u32_le();
                let height = b.get_u32_le();

                let name = b.split_to(16);
                let name = SHIFT_JIS.decode_without_bom_handling_and_without_replacement(&name).unwrap_or_else(|| format!("{id:08X}").into());
                let filename = format!("{}.png", name.trim_end_matches('\0'));

                println!("dumping {filename}");

                let palette = b.split_off((width as usize) * (height as usize) * ((bpp / 8) as usize));
                ensure!(palette.len() == 4 << bpp);

                let mut plte = Vec::with_capacity(3 << bpp);
                let mut trns = Vec::with_capacity(1 << bpp);

                for c in palette.chunks_exact(4) {
                    let [r, g, b, a] = c.try_into()?;
                    plte.extend_from_slice(&[r, g, b]);
                    trns.push(a);
                }

                ensure!(plte.len() == 3 << bpp);
                ensure!(trns.len() == 1 << bpp);

                for pix in b.iter_mut() {
                    // found using a PCSX2 texture dump, imageio, and a *lot* of guesswork
                    if pix.checked_sub(8).is_some_and(|z| z % 32 < 16) {
                        *pix ^= 24
                    }
                }

                let mut out = png::Encoder::new(File::create(filename)?, width, height);
                out.set_color(png::ColorType::Indexed);
                out.set_filter(png::FilterType::Paeth);
                out.set_depth(png::BitDepth::from_u8(bpp.try_into()?).context("bruh")?);
                out.set_palette(plte);
                out.set_trns(trns);
                let mut out = out.write_header()?;
                out.write_image_data(&b)?;
                out.finish()?;

                Ok(())
            })().unwrap());

            Ok(())
        })
    })
}