//! iNES cartridges. Mapper 0 (NROM) only: 16 or 32 KB of program ROM at
//! $8000 (16 KB mirrored), 8 KB of pattern ROM (or RAM, when the file has
//! none), 8 KB of work RAM at $6000, and fixed name-table mirroring.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mirroring {
    /// $2000 = $2400 and $2800 = $2C00: for games that scroll vertically.
    Horizontal,
    /// $2000 = $2800 and $2400 = $2C00: for games that scroll sideways.
    Vertical,
}

#[derive(Clone, Debug)]
pub struct Cart {
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
    pub chr_ram: bool,
    pub mirroring: Mirroring,
    pub mapper: u8,
    pub ram: Vec<u8>,
}

impl Cart {
    /// Read an iNES file.
    pub fn load(bytes: &[u8]) -> Result<Cart, String> {
        if bytes.len() < 16 || &bytes[0..4] != b"NES\x1A" {
            return Err("not an iNES file (no NES header)".into());
        }
        let prg_banks = bytes[4] as usize;
        let chr_banks = bytes[5] as usize;
        let flags6 = bytes[6];
        let flags7 = bytes[7];
        let mapper = (flags7 & 0xF0) | (flags6 >> 4);
        if mapper != 0 {
            return Err(format!("mapper {mapper} is not supported yet (only mapper 0, NROM)"));
        }
        if flags6 & 0x08 != 0 {
            return Err("four-screen name tables are not supported".into());
        }
        let mirroring = if flags6 & 1 != 0 { Mirroring::Vertical } else { Mirroring::Horizontal };
        let start = 16 + if flags6 & 0x04 != 0 { 512 } else { 0 };
        let prg_len = prg_banks * 0x4000;
        let chr_len = chr_banks * 0x2000;
        if prg_banks == 0 || bytes.len() < start + prg_len + chr_len {
            return Err(format!("the file is cut short: it should hold {} KB of program and {} KB of graphics", prg_len / 1024, chr_len / 1024));
        }
        let prg = bytes[start..start + prg_len].to_vec();
        let (chr, chr_ram) = if chr_len == 0 { (vec![0; 0x2000], true) } else { (bytes[start + prg_len..start + prg_len + chr_len].to_vec(), false) };
        Ok(Cart { prg, chr, chr_ram, mirroring, mapper, ram: vec![0; 0x2000] })
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.ram[(addr - 0x6000) as usize],
            0x8000..=0xFFFF => self.prg[(addr as usize - 0x8000) % self.prg.len()],
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, v: u8) {
        if let 0x6000..=0x7FFF = addr {
            self.ram[(addr - 0x6000) as usize] = v;
        }
    }

    pub fn chr_read(&self, addr: u16) -> u8 {
        self.chr[(addr & 0x1FFF) as usize]
    }

    pub fn chr_write(&mut self, addr: u16, v: u8) {
        if self.chr_ram {
            self.chr[(addr & 0x1FFF) as usize] = v;
        }
    }
}
