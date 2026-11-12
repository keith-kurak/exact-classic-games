//! PNG in and out. Writing: sprite sheets as 8-bit RGBA with stored deflate
//! blocks. Reading: [`decode`] takes the common non-interlaced kinds (grey,
//! RGB, indexed, grey+alpha, RGBA; 8-bit, and 1–8-bit indexed) to RGBA, so a
//! game can draw its sprites in a paint program; [`Image::art`] turns a
//! region into this engine's text art in a sprite's three colours.

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

/// A decoded picture: RGBA, row by row.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Read a PNG (not interlaced; 16-bit samples are refused).
pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    if bytes.len() < 8 || bytes[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Err("not a PNG".into());
    }
    let (mut width, mut height, mut depth, mut kind, mut interlace) = (0usize, 0usize, 0u8, 0u8, 0u8);
    let mut plte: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat = Vec::new();
    let mut at = 8;
    while at + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let tag = &bytes[at + 4..at + 8];
        let body = bytes.get(at + 8..at + 8 + len).ok_or("truncated PNG")?;
        match tag {
            b"IHDR" => {
                width = u32::from_be_bytes(body[0..4].try_into().unwrap()) as usize;
                height = u32::from_be_bytes(body[4..8].try_into().unwrap()) as usize;
                depth = body[8];
                kind = body[9];
                interlace = body[12];
            }
            b"PLTE" => plte = body.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
            b"tRNS" => trns = body.to_vec(),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        at += 12 + len;
    }
    if interlace != 0 {
        return Err("interlaced PNGs are not supported".into());
    }
    let channels = match kind {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        _ => return Err(format!("PNG colour type {kind} is not supported")),
    };
    if depth == 16 || (kind != 3 && depth != 8) || (kind == 3 && ![1, 2, 4, 8].contains(&depth)) {
        return Err(format!("PNG bit depth {depth} is not supported for colour type {kind}"));
    }
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat).map_err(|e| format!("PNG data: {e:?}"))?;
    let bits = channels * depth as usize;
    let stride = (width * bits + 7) / 8;
    let bpp = ((bits + 7) / 8).max(1);
    let mut rows = vec![0u8; stride * height];
    for y in 0..height {
        let line = raw.get(y * (stride + 1)..(y + 1) * (stride + 1)).ok_or("PNG data too short")?;
        let (filter, src) = (line[0], &line[1..]);
        for x in 0..stride {
            let a = if x >= bpp { rows[y * stride + x - bpp] } else { 0 };
            let b = if y > 0 { rows[(y - 1) * stride + x] } else { 0 };
            let c = if y > 0 && x >= bpp { rows[(y - 1) * stride + x - bpp] } else { 0 };
            let predict = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => {
                    let p = a as i16 + b as i16 - c as i16;
                    let (pa, pb, pc) = ((p - a as i16).abs(), (p - b as i16).abs(), (p - c as i16).abs());
                    if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
                }
                f => return Err(format!("PNG filter {f}")),
            };
            rows[y * stride + x] = src[x].wrapping_add(predict);
        }
    }
    let mut rgba = vec![0u8; width * height * 4];
    for y in 0..height {
        let row = &rows[y * stride..(y + 1) * stride];
        for x in 0..width {
            let px = match kind {
                3 => {
                    let per = 8 / depth as usize;
                    let byte = row[x / per];
                    let shift = 8 - depth as usize * (x % per + 1);
                    let i = ((byte >> shift) & ((1u16 << depth) - 1) as u8) as usize;
                    let c = plte.get(i).copied().unwrap_or([0, 0, 0]);
                    [c[0], c[1], c[2], trns.get(i).copied().unwrap_or(255)]
                }
                0 => [row[x], row[x], row[x], 255],
                4 => [row[x * 2], row[x * 2], row[x * 2], row[x * 2 + 1]],
                2 => [row[x * 3], row[x * 3 + 1], row[x * 3 + 2], 255],
                _ => [row[x * 4], row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3]],
            };
            rgba[(y * width + x) * 4..(y * width + x) * 4 + 4].copy_from_slice(&px);
        }
    }
    Ok(Image { width, height, rgba })
}

impl Image {
    /// The `w`×`h` region at (`x`, `y`) as text art for `parse_art` and
    /// `Metasprite::from_art`: transparent pixels (alpha under half) are `.`,
    /// and every other pixel is the nearest of `colours`, three 2C02 indices
    /// (a sprite palette's colours 1–3), as `1`–`3`. Draw a sheet in exactly
    /// those colours and it maps exactly.
    pub fn art(&self, x: usize, y: usize, w: usize, h: usize, colours: [u8; 3]) -> String {
        let rgb = colours.map(|c| crate::NES_PALETTE[(c & 0x3F) as usize]);
        let mut out = String::with_capacity((w + 1) * h);
        for yy in y..y + h {
            for xx in x..x + w {
                let i = (yy * self.width + xx) * 4;
                let p = self.rgba.get(i..i + 4).unwrap_or(&[0, 0, 0, 0]);
                if p[3] < 128 {
                    out.push('.');
                    continue;
                }
                let d = |c: [u8; 3]| -> i32 { (0..3).map(|k| (p[k] as i32 - c[k] as i32).pow(2)).sum() };
                let best = (0..3).min_by_key(|&k| d(rgb[k])).unwrap();
                out.push((b'1' + best as u8) as char);
            }
            out.push('\n');
        }
        out
    }

    /// The region as background art: every pixel the nearest of a whole
    /// background palette's four colours (`palette[0]`, the backdrop, as
    /// `.`); transparent pixels are backdrop too.
    pub fn bg_art(&self, x: usize, y: usize, w: usize, h: usize, palette: [u8; 4]) -> String {
        let rgb = palette.map(|c| crate::NES_PALETTE[(c & 0x3F) as usize]);
        let mut out = String::with_capacity((w + 1) * h);
        for yy in y..y + h {
            for xx in x..x + w {
                let i = (yy * self.width + xx) * 4;
                let p = self.rgba.get(i..i + 4).unwrap_or(&[0, 0, 0, 0]);
                if p[3] < 128 {
                    out.push('.');
                    continue;
                }
                let d = |c: [u8; 3]| -> i32 { (0..3).map(|k| (p[k] as i32 - c[k] as i32).pow(2)).sum() };
                let best = (0..4).min_by_key(|&k| d(rgb[k])).unwrap();
                out.push(if best == 0 { '.' } else { (b'0' + best as u8) as char });
            }
            out.push('\n');
        }
        out
    }
}
