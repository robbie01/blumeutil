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
    // swap the two bits in the middle, for some reason
    // found using a PCSX2 texture dump, imageio, numpy, and a *lot* of guesswork
    (id & 0b11100111) | ((id & 0b00010000) >> 1) | ((id & 0b00001000) << 1)
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
                let filename = SHIFT_JIS.decode_without_bom_handling_and_without_replacement(&name).map_or_else(|| format!("{id:08X}.png"), |name| format!("{id:08X} - {}.png", name.trim_end_matches('\0')));

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

                let mut info = png::Info::default();
                info.width = width;
                info.height = height;
                info.color_type = png::ColorType::Indexed;
                info.bit_depth = png::BitDepth::from_u8(bpp.try_into()?).context("bruh")?;
                info.palette = Some(plte.into());
                info.trns = Some(trns.into());
                let mut out = png::Encoder::with_info(File::create(filename)?, info)?;
                out.set_filter(png::FilterType::Paeth);
                let mut out = out.write_header()?;
                out.write_image_data(&img)?;
                out.finish()?;

                Ok(())
            })().unwrap());

            Ok(())
        })
    })
}