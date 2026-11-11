//! A small PNG writer for sprite sheets: 8-bit RGBA, stored (uncompressed)
//! deflate blocks, no dependencies.

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// A PNG of `width`×`height` RGBA pixels.
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width * 4 + 1) as usize * height as usize);
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * width as usize * 4..(y + 1) * width as usize * 4]);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push(if i + 1 == blocks.len() { 1 } else { 0 });
        let len = block.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// A pattern table as a sheet, 16 tiles a row, each tile in the given
/// palette (four 2C02 indices; colour 0 drawn as transparent when `clear`).
pub fn sheet(table: &crate::PatternTable, palette: [u8; 4], clear: bool) -> Vec<u8> {
    let cols = 16usize;
    let rows = table.tiles.len().div_ceil(cols).max(1);
    let (w, h) = (cols * 8, rows * 8);
    let mut rgba = vec![0u8; w * h * 4];
    for (i, t) in table.tiles.iter().enumerate() {
        let (tx, ty) = (i % cols, i / cols);
        for y in 0..8 {
            for x in 0..8 {
                let c = t.0[y][x];
                let rgb = crate::NES_PALETTE[(palette[c as usize] & 0x3F) as usize];
                let o = ((ty * 8 + y) * w + tx * 8 + x) * 4;
                let a = if clear && c == 0 { 0 } else { 255 };
                rgba[o..o + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
            }
        }
    }
    encode(w as u32, h as u32, &rgba)
}
