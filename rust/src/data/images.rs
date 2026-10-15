//! Multi-image LZW files (*.16 other than TILES.16, *.4, *.BIT): header (u16 count, offsets[count]),
//! each image (u16 width, u16 height, rows). Offsets are u32 except in ITEMS.*, MON*.*, *.BIT (u16).
//! ITEMS/MON images are followed by a 1-bpp mask (u16 w, u16 h, bits).
use crate::lzw;

#[derive(Clone, Debug)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// One byte per pixel, colour index.
    pub pixels: Vec<u8>,
    /// Optional mask: true = masked (do not draw).
    pub mask: Option<Vec<bool>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Depth { Bpp1, Bpp2, Bpp4 }

impl Depth {
    pub fn row_bytes(self, width: usize) -> usize {
        match self {
            Depth::Bpp4 => (width + 7) / 8 * 4,
            Depth::Bpp2 => (width + 3) / 4,
            Depth::Bpp1 => (width + 7) / 8,
        }
    }
    pub fn unpack_row(self, row: &[u8], width: usize, out: &mut Vec<u8>) {
        for x in 0..width {
            let v = match self {
                Depth::Bpp4 => { let b = row[x / 2]; if x % 2 == 0 { b >> 4 } else { b & 15 } }
                Depth::Bpp2 => { let b = row[x / 4]; (b >> (6 - 2 * (x % 4))) & 3 }
                Depth::Bpp1 => { let b = row[x / 8]; (b >> (7 - (x % 8))) & 1 }
            };
            out.push(v);
        }
    }
}

pub struct ImageFile {
    pub images: Vec<Option<Image>>,
}

fn rd16(d: &[u8], o: usize) -> usize { u16::from_le_bytes([d[o], d[o + 1]]) as usize }
fn rd32(d: &[u8], o: usize) -> usize { u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) as usize }

impl ImageFile {
    /// `offsets16`: true for ITEMS.*, MON*.* and *.BIT; `masked`: true for ITEMS.* and MON*.*.
    pub fn load(raw: &[u8], depth: Depth, offsets16: bool, masked: bool) -> Result<ImageFile, String> {
        let d = lzw::decompress_file(raw)?;
        Self::parse(&d, depth, offsets16, masked)
    }
    pub fn parse(d: &[u8], depth: Depth, offsets16: bool, masked: bool) -> Result<ImageFile, String> {
        let count = rd16(d, 0);
        let mut images = Vec::with_capacity(count);
        for i in 0..count {
            let off = if offsets16 { rd16(d, 2 + i * 2) } else { rd32(d, 2 + i * 4) };
            if off == 0 { images.push(None); continue; }
            let w = rd16(d, off);
            let h = rd16(d, off + 2);
            let rb = depth.row_bytes(w);
            let mut pixels = Vec::with_capacity(w * h);
            let mut p = off + 4;
            for _ in 0..h {
                if p + rb > d.len() { return Err(format!("image {} truncated", i)); }
                depth.unpack_row(&d[p..p + rb], w, &mut pixels);
                p += rb;
            }
            let mask = if masked {
                let mw = rd16(d, p); let mh = rd16(d, p + 2);
                let nbits = mw * mh;
                let mut m = Vec::with_capacity(nbits);
                let mb = &d[p + 4..];
                for b in 0..nbits { m.push((mb[b / 8] >> (7 - (b % 8))) & 1 != 0); }
                Some(m)
            } else { None };
            images.push(Some(Image { width: w, height: h, pixels, mask }));
        }
        Ok(ImageFile { images })
    }
}
