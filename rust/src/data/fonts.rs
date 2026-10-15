//! Fixed 8x8 fonts: IBM.CH / RUNES.CH (1024 bytes = 128 glyphs x 8 rows, 1 bpp, MSB = leftmost pixel).
//! *.HCS files (3072 bytes) are the Hercules variants.
pub struct Font8x8 {
    pub glyphs: Vec<[u8; 8]>,
}
impl Font8x8 {
    pub fn from_ch(raw: &[u8]) -> Font8x8 {
        let n = raw.len() / 8;
        let mut glyphs = Vec::with_capacity(n);
        for i in 0..n {
            let mut g = [0u8; 8];
            g.copy_from_slice(&raw[i * 8..i * 8 + 8]);
            glyphs.push(g);
        }
        Font8x8 { glyphs }
    }
}
