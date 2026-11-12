//! A small sound driver, the way NES games made music: once a frame (60
//! times a second) it writes each channel's registers from its instrument and
//! the song's notes. Songs are written as text, one token a row per channel:
//!
//! - a note, `C4`, `F#3`, `Bb5` (middle C is C4), sounds until the next;
//! - `.` holds what is sounding; `-` releases it (the instrument's release
//!   envelope, then silence);
//! - `@n` before a note switches to instrument `n`: `@1E5`;
//! - on the noise channel, a drum's letter from the song's kit: `K`, `S`, `H`…
//!
//! A channel's row list repeats when it runs out, so a short bass line can
//! loop under a long melody.

use crate::{note_hz, pulse_period, triangle_period, Apu, CPU_HZ, FRAME_HZ};

/// How a pulse or triangle note sounds, frame by frame.
#[derive(Clone, Debug)]
pub struct Instrument {
    /// Pulse duty: 0 (12.5%), 1 (25%), 2 (50%), 3 (75%).
    pub duty: u8,
    /// Volume 0–15 each frame from the note's start; the last value holds.
    pub volume: &'static [u8],
    /// Volume each frame after a release (`-`); then silence.
    pub release: &'static [u8],
    /// Semitones added each frame, looping (an arpeggio, the chiptune chord).
    pub arpeggio: &'static [i8],
    /// Vibrato: depth in timer steps and speed in frames a cycle, after a delay.
    pub vibrato: (u16, u16, u16),
    /// A hardware sweep ($4001) for each new note, or 0 for none.
    pub sweep: u8,
}

impl Instrument {
    pub const fn plain(duty: u8, volume: &'static [u8], release: &'static [u8]) -> Instrument {
        Instrument { duty, volume, release, arpeggio: &[], vibrato: (0, 0, 0), sweep: 0 }
    }
}

/// A noise-channel drum: its noise period (0–15, high is low), short mode
/// (metallic), and volume each frame.
#[derive(Clone, Debug)]
pub struct Drum {
    pub name: char,
    pub period: u8,
    pub short: bool,
    pub volume: &'static [u8],
    /// Period each frame after the first, to sweep a hit down (or empty).
    pub periods: &'static [u8],
}

/// A song: four channels of rows (pulse 1, pulse 2, triangle, noise).
#[derive(Clone, Debug)]
pub struct Song {
    pub name: &'static str,
    /// Frames each row lasts (6 at 60 Hz is 150 BPM in sixteenths).
    pub speed: u32,
    pub rows: [&'static str; 4],
    pub instruments: &'static [Instrument],
    pub drums: &'static [Drum],
}

#[derive(Clone, Debug, Default)]
struct Voice {
    note: Option<i32>,
    inst: usize,
    age: u32,
    released: Option<u32>,
    last_high: Option<u8>,
    drum: Option<usize>,
}

/// What a channel shows on a row: its note's name (or a drum's letter, or
/// "") and its volume, 0–15.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub label: String,
    pub volume: u8,
}

/// A rendered song: its samples, and each row's cells for the four channels.
pub struct Rendered {
    pub samples: Vec<f32>,
    pub rows: Vec<[Cell; 4]>,
    pub row_ms: f64,
}

fn parse_note(s: &str) -> Option<i32> {
    let mut chars = s.chars();
    let letter = chars.next()?;
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (acc, oct) = if let Some(r) = rest.strip_prefix('#') { (1, r) } else if let Some(r) = rest.strip_prefix('b') { (-1, r) } else { (0, rest.as_str()) };
    let octave: i32 = oct.parse().ok()?;
    Some(12 * (octave + 1) + base + acc)
}

fn note_name(n: i32) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", NAMES[n.rem_euclid(12) as usize], n.div_euclid(12) - 1)
}

/// Split a channel's text into tokens, one a row.
pub fn tokens(text: &str) -> Vec<&str> {
    text.split_whitespace().filter(|t| !t.starts_with("|")).collect()
}

/// Render `song` for `rows` rows, with the APU making `rate` samples a second.
pub fn render(song: &Song, rows: usize, rate: u32) -> Rendered {
    let mut apu = Apu::new(rate);
    apu.write(0x4015, 0x0F);
    apu.write(0x4017, 0x40);
    apu.write(0x4001, 0x08);
    apu.write(0x4005, 0x08);
    let lines: Vec<Vec<&str>> = song.rows.iter().map(|r| tokens(r)).collect();
    let mut voices: [Voice; 4] = Default::default();
    let mut log = Vec::with_capacity(rows);
    let cycles_per_frame = CPU_HZ / FRAME_HZ;
    let mut owed = 0.0f64;
    for row in 0..rows {
        // New tokens at the row's start.
        for ch in 0..4 {
            let line = &lines[ch];
            if line.is_empty() {
                continue;
            }
            let tok = line[row % line.len()];
            let v = &mut voices[ch];
            let mut t = tok;
            if let Some(rest) = t.strip_prefix('@') {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                v.inst = digits.parse().unwrap_or(0);
                t = &rest[digits.len()..];
            }
            match t {
                "." | "" => {}
                "-" => {
                    if v.released.is_none() && (v.note.is_some() || v.drum.is_some()) {
                        v.released = Some(0);
                    }
                }
                _ if ch == 3 => {
                    let c = t.chars().next().unwrap();
                    if let Some(d) = song.drums.iter().position(|d| d.name == c) {
                        v.drum = Some(d);
                        v.age = 0;
                        v.released = None;
                        let dr = &song.drums[d];
                        apu.write(0x400E, (if dr.short { 0x80 } else { 0 }) | dr.period);
                        apu.write(0x400F, 0x08);
                    }
                }
                _ => {
                    if let Some(n) = parse_note(t) {
                        v.note = Some(n);
                        v.age = 0;
                        v.released = None;
                        v.last_high = None;
                        let inst = &song.instruments[v.inst.min(song.instruments.len() - 1)];
                        if ch < 2 {
                            apu.write(0x4001 + ch as u16 * 4, if inst.sweep != 0 { inst.sweep } else { 0x08 });
                        }
                    }
                }
            }
        }
        let mut cells: [Cell; 4] = Default::default();
        for f in 0..song.speed {
            for ch in 0..4 {
                let v = &mut voices[ch];
                let vol = if ch == 3 {
                    match v.drum {
                        Some(d) => {
                            let dr = &song.drums[d];
                            let a = v.age as usize;
                            if a > 0 && !dr.periods.is_empty() {
                                let p = dr.periods[(a - 1).min(dr.periods.len() - 1)];
                                apu.write(0x400E, (if dr.short { 0x80 } else { 0 }) | p);
                            }
                            dr.volume.get(a).copied().unwrap_or(0)
                        }
                        None => 0,
                    }
                } else {
                    let inst = &song.instruments[v.inst.min(song.instruments.len() - 1)];
                    match (v.note, v.released) {
                        (None, _) => 0,
                        (Some(_), None) => inst.volume[(v.age as usize).min(inst.volume.len() - 1)],
                        (Some(_), Some(r)) => inst.release.get(r as usize).copied().unwrap_or(0),
                    }
                };
                if ch == 3 {
                    apu.write(0x400C, 0x30 | vol);
                } else if let Some(n) = v.note {
                    let inst = &song.instruments[v.inst.min(song.instruments.len() - 1)];
                    let arp = if inst.arpeggio.is_empty() { 0 } else { inst.arpeggio[v.age as usize % inst.arpeggio.len()] as i32 };
                    let hz = note_hz(n + arp);
                    let mut period = if ch == 2 { triangle_period(hz) } else { pulse_period(hz) } as i32;
                    let (depth, speed, delay) = inst.vibrato;
                    if depth > 0 && v.age >= delay as u32 {
                        let phase = ((v.age - delay as u32) % speed as u32) as f64 / speed as f64;
                        period += ((phase * std::f64::consts::TAU).sin() * depth as f64).round() as i32;
                    }
                    let period = period.clamp(0, 2047) as u16;
                    let base = 0x4000 + ch as u16 * 4;
                    if ch == 2 {
                        apu.write(0x4008, if vol > 0 { 0xFF } else { 0x80 });
                    } else {
                        apu.write(base, (inst.duty << 6) | 0x30 | vol);
                    }
                    let swept = ch < 2 && inst.sweep != 0 && v.age > 0;
                    if !swept {
                        apu.write(base + 2, (period & 0xFF) as u8);
                        // The high byte restarts the wave (a click), so only
                        // when it changes, as careful drivers did.
                        let high = (period >> 8) as u8 | 0x08;
                        if v.last_high != Some(high) {
                            apu.write(base + 3, high);
                            v.last_high = Some(high);
                        }
                    }
                } else if ch == 2 {
                    apu.write(0x4008, 0x80);
                } else {
                    apu.write(0x4000 + ch as u16 * 4, 0x30);
                }
                if f == 0 {
                    let label = if ch == 3 {
                        v.drum.filter(|_| vol > 0).map(|d| song.drums[d].name.to_string()).unwrap_or_default()
                    } else {
                        v.note.filter(|_| vol > 0).map(note_name).unwrap_or_default()
                    };
                    cells[ch] = Cell { label, volume: if ch == 2 && vol > 0 { 15 } else { vol } };
                }
                v.age += 1;
                if let Some(r) = v.released.as_mut() {
                    *r += 1;
                }
            }
            owed += cycles_per_frame;
            let n = owed.floor();
            owed -= n;
            apu.run(n as u32);
        }
        log.push(cells);
    }
    Rendered { samples: apu.take_samples(), rows: log, row_ms: song.speed as f64 * 1000.0 / FRAME_HZ }
}

/// Render raw register writes: `(frame, address, value)`, for `frames`
/// frames: a sound effect written the way a game's code would.
pub fn render_writes(writes: &[(u32, u16, u8)], frames: u32, rate: u32) -> Vec<f32> {
    let mut apu = Apu::new(rate);
    apu.write(0x4015, 0x0F);
    apu.write(0x4017, 0x40);
    let cycles_per_frame = CPU_HZ / FRAME_HZ;
    let mut owed = 0.0f64;
    for f in 0..frames {
        for &(at, addr, v) in writes.iter().filter(|w| w.0 == f) {
            let _ = at;
            apu.write(addr, v);
        }
        owed += cycles_per_frame;
        let n = owed.floor();
        owed -= n;
        apu.run(n as u32);
    }
    apu.take_samples()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_parse() {
        assert_eq!(parse_note("A4"), Some(69));
        assert_eq!(parse_note("C4"), Some(60));
        assert_eq!(parse_note("F#3"), Some(54));
        assert_eq!(parse_note("Bb2"), Some(46));
        assert_eq!(note_name(61), "C#4");
    }

    #[test]
    fn a_song_renders_rows_and_samples() {
        static LEAD: [Instrument; 1] = [Instrument::plain(2, &[12, 10, 8], &[4, 2])];
        static KIT: [Drum; 1] = [Drum { name: 'H', period: 1, short: false, volume: &[8, 4, 2], periods: &[] }];
        let song = Song { name: "t", speed: 6, rows: ["C4 . E4 -", "", "C3 . . .", "H . H ."], instruments: &LEAD, drums: &KIT };
        let r = render(&song, 4, 22050);
        assert_eq!(r.rows.len(), 4);
        assert_eq!(r.rows[0][0].label, "C4");
        assert_eq!(r.rows[2][0].label, "E4");
        assert_eq!(r.rows[0][2].label, "C3");
        assert_eq!(r.rows[0][3].label, "H");
        let expected = (4.0 * 6.0 / FRAME_HZ * 22050.0) as usize;
        assert!((r.samples.len() as i64 - expected as i64).abs() < 50, "{} {}", r.samples.len(), expected);
        assert!(r.samples.iter().any(|s| s.abs() > 0.05));
    }
}
