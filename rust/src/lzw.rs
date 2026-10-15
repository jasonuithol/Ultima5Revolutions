//! LZW decompressor for Ultima V graphics files (*.16, *.4, *.BIT, PROPORT.PCS).
//! Files are stored as (u32 uncompressed_length, compressed bytes). Variable code width 9..12 bits,
//! LSB-first bit packing, CLEAR=256, END=257.

pub fn decompress_file(raw: &[u8]) -> Result<Vec<u8>, String> {
    if raw.len() < 4 {
        return Err("LZW file too short".into());
    }
    let expected = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    let out = decompress(&raw[4..]);
    if out.len() != expected {
        return Err(format!("LZW length mismatch: got {} expected {}", out.len(), expected));
    }
    Ok(out)
}

pub fn decompress(data: &[u8]) -> Vec<u8> {
    const CLEAR: usize = 256;
    const END: usize = 257;
    let mut code_size = 9u32;
    let mut dict: Vec<Vec<u8>> = (0..256).map(|i| vec![i as u8]).collect();
    dict.push(vec![]);
    dict.push(vec![]);
    let mut result = Vec::new();
    let mut bits: u32 = 0;
    let mut nbits = 0u32;
    let mut pos = 0usize;
    let mut prev: Option<usize> = None;
    loop {
        while nbits < code_size && pos < data.len() {
            bits |= (data[pos] as u32) << nbits;
            nbits += 8;
            pos += 1;
        }
        if nbits < code_size {
            break;
        }
        let code = (bits & ((1 << code_size) - 1)) as usize;
        bits >>= code_size;
        nbits -= code_size;
        if code == CLEAR {
            dict.truncate(258);
            code_size = 9;
            prev = None;
            continue;
        }
        if code == END {
            break;
        }
        let entry: Vec<u8> = if code < dict.len() {
            dict[code].clone()
        } else if code == dict.len() {
            match prev {
                Some(p) => {
                    let mut e = dict[p].clone();
                    e.push(dict[p][0]);
                    e
                }
                None => break,
            }
        } else {
            break;
        };
        result.extend_from_slice(&entry);
        if let Some(p) = prev {
            let mut ne = dict[p].clone();
            ne.push(entry[0]);
            dict.push(ne);
            if dict.len() == (1usize << code_size) && code_size < 12 {
                code_size += 1;
            }
        }
        prev = Some(code);
    }
    result
}
