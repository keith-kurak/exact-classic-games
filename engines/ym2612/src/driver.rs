//! A small sound driver, the way Mega Drive games made music: once a frame
//! (60 times a second) it keys notes on and off and writes pitches, from
//! each channel's FM patch and the song's notes; on channel 6 it can feed
//! drum samples to the DAC instead, a byte at a time, as a Z80 driver did.
//!
//! Songs are text, one token a row per channel:
//!
//! - a note, `C4`, `F#3`, `Bb5` (middle C is C4), keyed on until the next;
//! - `.` holds what is sounding; `-` keys it off (the patch's release);
//! - `@n` before a note switches to instrument `n`: `@1E5`;
//! - on channel 6, with a drum kit, a drum's letter: `K`, `S`, `H`…
//!
//! A channel's row list repeats when it runs out; `|` marks bars and is
//! skipped.

use crate::drums::DAC_DIVIDER;
use crate::{fnum_for, note_hz, Ym2612, CHIP_HZ};

pub const FRAME_HZ: f64 = 60.0;

/// One operator's settings.
#[derive(Clone, Copy, Debug)]
pub struct Op {
    /// Multiple: 0 is half the pitch, 1–15 times it.
    pub mul: u8,
    /// Total level: 0 loudest, 127 silent, in 0.75 dB steps.
    pub tl: u8,
    /// Attack, first decay and second decay rates (0–31), the sustain level
    /// (0–15, 3 dB steps) and the release rate (0–15).
    pub ar: u8,
    pub d1r: u8,
    pub d2r: u8,
    pub sl: u8,
    pub rr: u8,
    /// Detune (0–7: 1–3 up, 5–7 down), rate key scaling (0–3), LFO tremolo
    /// on, and SSG-EG (8–15, or 0).
    pub dt: u8,
    pub ks: u8,
    pub am: bool,
    pub ssg: u8,
}

impl Op {
    pub const fn new(mul: u8, tl: u8, ar: u8, d1r: u8, d2r: u8, sl: u8, rr: u8) -> Op {
        Op { mul, tl, ar, d1r, d2r, sl, rr, dt: 0, ks: 0, am: false, ssg: 0 }
    }
    pub const fn dt(self, dt: u8) -> Op {
        Op { dt, ..self }
    }
    pub const fn ks(self, ks: u8) -> Op {
        Op { ks, ..self }
    }
    pub const fn am(self) -> Op {
        Op { am: true, ..self }
    }
    pub const fn ssg(self, ssg: u8) -> Op {
        Op { ssg, ..self }
    }
}

/// An FM patch: the algorithm, operator 1's feedback, the four operators
/// (1, 2, 3, 4), and the LFO's depths for the channel.
#[derive(Clone, Copy, Debug)]
pub struct Patch {
    pub name: &'static str,
    pub algorithm: u8,
    pub feedback: u8,
    pub ops: [Op; 4],
    pub ams: u8,
    pub pms: u8,
}

/// Which operators an algorithm sends to the output.
pub fn carriers(algorithm: u8) -> [bool; 4] {
    match algorithm {
        0..=3 => [false, false, false, true],
        4 => [false, true, false, true],
        5 | 6 => [false, true, true, true],
        _ => [true; 4],
    }
}

/// An instrument: a patch, how much quieter it plays (total level steps added
/// to its carriers), software vibrato (cents deep, frames a cycle, frames of
/// delay), and a transposition in semitones.
#[derive(Clone, Copy, Debug)]
pub struct Instrument {
    pub patch: Patch,
    pub volume: u8,
    pub vibrato: (f32, u32, u32),
    pub transpose: i32,
}

impl Instrument {
    pub const fn plain(patch: Patch) -> Instrument {
        Instrument { patch, volume: 0, vibrato: (0.0, 0, 0), transpose: 0 }
    }
    pub const fn volume(self, volume: u8) -> Instrument {
        Instrument { volume, ..self }
    }
    pub const fn vibrato(self, cents: f32, frames: u32, delay: u32) -> Instrument {
        Instrument { vibrato: (cents, frames, delay), ..self }
    }
    pub const fn transpose(self, transpose: i32) -> Instrument {
        Instrument { transpose, ..self }
    }
}

/// A DAC drum: its letter and its sample.
#[derive(Clone, Copy, Debug)]
pub struct Drum {
    pub name: char,
    pub make: fn() -> Vec<u8>,
}

/// A song: six channels of rows. With a drum kit, channel 6 plays it
/// through the DAC; without one, it is a sixth FM channel.
#[derive(Clone, Debug)]
pub struct Song {
    pub name: &'static str,
    /// Frames each row lasts (6 at 60 Hz is 150 BPM in sixteenths).
    pub speed: u32,
    pub rows: [&'static str; 6],
    pub instruments: &'static [Instrument],
    pub drums: &'static [Drum],
    /// Each channel's panning: `$C0` both, `$80` left, `$40` right.
    pub pan: [u8; 6],
    /// The LFO: 0 off, or 8 + its frequency (0–7).
    pub lfo: u8,
}

/// What a channel shows on a row: its note's name (or a drum's letter, or
/// ""), its loudest level that row (0–15) and its instrument.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    pub label: String,
    pub volume: u8,
    pub instrument: usize,
}

/// A rendered song: its samples (interleaved stereo), each row's cells for
/// the six channels, and how long a row lasts.
pub struct Rendered {
    pub samples: Vec<f32>,
    pub rows: Vec<[Cell; 6]>,
    pub row_ms: f64,
}

pub fn parse_note(s: &str) -> Option<i32> {
    let mut chars = s.chars();
    let base = match chars.next()? {
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

pub fn note_name(n: i32) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", NAMES[n.rem_euclid(12) as usize], n.div_euclid(12) - 1)
}

/// Split a channel's text into tokens, one a row.
pub fn tokens(text: &str) -> Vec<&str> {
    text.split_whitespace().filter(|t| !t.starts_with('|')).collect()
}

fn port_and_slot(ch: usize) -> (u8, u8) {
    ((ch / 3) as u8, (ch % 3) as u8)
}

/// The key-on register's channel number.
fn key_channel(ch: usize) -> u8 {
    if ch < 3 { ch as u8 } else { ch as u8 + 1 }
}

/// Register offset for operators 1–4: the chip's slots run 1, 3, 2, 4.
const OP_OFFSET: [u8; 4] = [0, 8, 4, 12];

/// Anything register writes go to: the chip, or a script of them.
pub trait Sink {
    fn put(&mut self, port: u8, addr: u8, data: u8);
}

impl Sink for Ym2612 {
    fn put(&mut self, port: u8, addr: u8, data: u8) {
        self.write(port, addr, data);
    }
}

/// Register writes recorded frame by frame: a sound effect, the way game
/// code wrote one. `at` moves to a frame; the rest record writes there.
#[derive(Default, Clone, Debug)]
pub struct Script {
    pub writes: Vec<(u32, u8, u8, u8)>,
    pub frame: u32,
}

impl Sink for Script {
    fn put(&mut self, port: u8, addr: u8, data: u8) {
        self.writes.push((self.frame, port, addr, data));
    }
}

impl Script {
    pub fn at(&mut self, frame: u32) -> &mut Script {
        self.frame = frame;
        self
    }
    pub fn write(&mut self, port: u8, addr: u8, data: u8) -> &mut Script {
        self.put(port, addr, data);
        self
    }
    pub fn load(&mut self, ch: usize, inst: &Instrument, pan: u8) -> &mut Script {
        load(self, ch, inst, pan);
        self
    }
    pub fn pitch(&mut self, ch: usize, block: u8, fnum: u16) -> &mut Script {
        pitch(self, ch, block, fnum);
        self
    }
    /// Pitch channel `ch` to a MIDI note and key it on.
    pub fn note(&mut self, ch: usize, midi: i32) -> &mut Script {
        let (b, f) = fnum_for(note_hz(midi));
        pitch(self, ch, b, f);
        key(self, ch, true);
        self
    }
    pub fn key(&mut self, ch: usize, on: bool) -> &mut Script {
        key(self, ch, on);
        self
    }
}

/// Write an instrument's patch to channel `ch`, panned `pan`.
pub fn load(chip: &mut impl Sink, ch: usize, inst: &Instrument, pan: u8) {
    let (port, c) = port_and_slot(ch);
    let p = &inst.patch;
    let carrier = carriers(p.algorithm);
    chip.put(port, 0xB0 + c, (p.feedback << 3) | p.algorithm);
    chip.put(port, 0xB4 + c, (pan & 0xC0) | (p.ams << 4) | p.pms);
    for (i, op) in p.ops.iter().enumerate() {
        let r = OP_OFFSET[i] + c;
        let tl = if carrier[i] { (op.tl as u32 + inst.volume as u32).min(127) as u8 } else { op.tl };
        chip.put(port, 0x30 + r, (op.dt << 4) | op.mul);
        chip.put(port, 0x40 + r, tl);
        chip.put(port, 0x50 + r, (op.ks << 6) | op.ar);
        chip.put(port, 0x60 + r, (if op.am { 0x80 } else { 0 }) | op.d1r);
        chip.put(port, 0x70 + r, op.d2r);
        chip.put(port, 0x80 + r, (op.sl << 4) | op.rr);
        chip.put(port, 0x90 + r, op.ssg);
    }
}

/// Set channel `ch`'s pitch.
pub fn pitch(chip: &mut impl Sink, ch: usize, block: u8, fnum: u16) {
    let (port, c) = port_and_slot(ch);
    chip.put(port, 0xA4 + c, (block << 3) | (fnum >> 8) as u8);
    chip.put(port, 0xA0 + c, fnum as u8);
}

pub fn key(chip: &mut impl Sink, ch: usize, on: bool) {
    chip.put(0, 0x28, if on { 0xF0 } else { 0 } | key_channel(ch));
}

#[derive(Clone, Debug, Default)]
struct Voice {
    note: Option<i32>,
    inst: usize,
    loaded: Option<usize>,
    age: u32,
    keyed: bool,
    pitch: (u8, u16),
}

/// Plays samples through the DAC while the chip runs.
#[derive(Default)]
struct Dac {
    sample: Vec<u8>,
    at: usize,
    tick: u32,
}

impl Dac {
    fn run(&mut self, chip: &mut Ym2612, n: u32) {
        for _ in 0..n {
            if self.tick % DAC_DIVIDER == 0 && self.at < self.sample.len() {
                chip.write(0, 0x2A, self.sample[self.at]);
                self.at += 1;
                if self.at == self.sample.len() {
                    chip.write(0, 0x2A, 0x80);
                }
            }
            self.tick += 1;
            chip.step();
        }
    }

    /// How much of the sample is left, 0–15.
    fn level(&self) -> u8 {
        if self.at >= self.sample.len() {
            0
        } else {
            (15 * (self.sample.len() - self.at) / self.sample.len()).max(1) as u8
        }
    }
}

/// Render `song` for `rows` rows, at `rate` samples a second.
pub fn render(song: &Song, rows: usize, rate: u32) -> Rendered {
    let mut chip = Ym2612::new(rate);
    let dac_drums = !song.drums.is_empty();
    chip.write(0, 0x22, song.lfo);
    chip.write(0, 0x2B, if dac_drums { 0x80 } else { 0 });
    if dac_drums {
        chip.write(1, 0xB6, song.pan[5] & 0xC0);
    }
    let samples: Vec<Vec<u8>> = song.drums.iter().map(|d| (d.make)()).collect();
    let lines: Vec<Vec<&str>> = song.rows.iter().map(|r| tokens(r)).collect();
    let mut voices: [Voice; 6] = Default::default();
    let mut dac = Dac::default();
    let mut drum_label = String::new();
    let mut log = Vec::with_capacity(rows);
    let per_frame = CHIP_HZ / FRAME_HZ;
    let mut owed = 0.0f64;
    let fm_channels = if dac_drums { 5 } else { 6 };
    for row in 0..rows {
        let mut cells: [Cell; 6] = Default::default();
        for ch in 0..6 {
            let line = &lines[ch];
            if line.is_empty() {
                continue;
            }
            let mut t = line[row % line.len()];
            let v = &mut voices[ch];
            if let Some(rest) = t.strip_prefix('@') {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                v.inst = digits.parse().unwrap_or(0);
                t = &rest[digits.len()..];
            }
            match t {
                "." | "" => {}
                "-" => {
                    if ch < fm_channels && v.keyed {
                        key(&mut chip, ch, false);
                        v.keyed = false;
                    }
                }
                _ if ch == 5 && dac_drums => {
                    let c = t.chars().next().unwrap_or(' ');
                    if let Some(d) = song.drums.iter().position(|d| d.name == c) {
                        dac.sample = samples[d].clone();
                        dac.at = 0;
                        drum_label = c.to_string();
                    }
                }
                _ => {
                    if let Some(n) = parse_note(t) {
                        let i = v.inst.min(song.instruments.len() - 1);
                        let inst = &song.instruments[i];
                        if v.keyed {
                            key(&mut chip, ch, false);
                        }
                        if v.loaded != Some(i) {
                            load(&mut chip, ch, inst, song.pan[ch]);
                            v.loaded = Some(i);
                        }
                        v.note = Some(n);
                        v.age = 0;
                        v.pitch = fnum_for(note_hz(n + inst.transpose));
                        pitch(&mut chip, ch, v.pitch.0, v.pitch.1);
                        key(&mut chip, ch, true);
                        v.keyed = true;
                    }
                }
            }
        }
        for _ in 0..song.speed {
            // Software vibrato: the F-number bent around the note's, in its block.
            for ch in 0..fm_channels {
                let v = &mut voices[ch];
                if let Some(_) = v.note {
                    let inst = &song.instruments[v.inst.min(song.instruments.len() - 1)];
                    let (cents, frames, delay) = inst.vibrato;
                    if cents > 0.0 && frames > 0 && v.age >= delay {
                        let phase = ((v.age - delay) % frames) as f64 / frames as f64;
                        let bend = 2f64.powf((phase * std::f64::consts::TAU).sin() * cents as f64 / 1200.0);
                        let f = ((v.pitch.1 as f64 * bend).round() as i64).clamp(0, 2047) as u16;
                        pitch(&mut chip, ch, v.pitch.0, f);
                    }
                    v.age += 1;
                }
            }
            owed += per_frame;
            let n = owed.floor();
            owed -= n;
            dac.run(&mut chip, n as u32);
            for ch in 0..6 {
                let level = if ch == 5 && dac_drums { dac.level() } else { chip.level(ch) };
                cells[ch].volume = cells[ch].volume.max(level);
            }
        }
        for ch in 0..6 {
            let v = &voices[ch];
            cells[ch].instrument = v.inst;
            if cells[ch].volume > 0 {
                cells[ch].label = if ch == 5 && dac_drums { drum_label.clone() } else { v.note.map(note_name).unwrap_or_default() };
            }
        }
        log.push(cells);
    }
    Rendered { samples: chip.take_samples(), rows: log, row_ms: song.speed as f64 * 1000.0 / FRAME_HZ }
}

/// Render raw register writes, `(frame, port, address, value)`, for
/// `frames` frames, with `dac` streamed to the DAC from the first frame: a
/// sound effect written the way a game's code would.
pub fn render_writes(writes: &[(u32, u8, u8, u8)], frames: u32, dac_sample: &[u8], rate: u32) -> Vec<f32> {
    let mut chip = Ym2612::new(rate);
    let mut dac = Dac { sample: dac_sample.to_vec(), at: 0, tick: 0 };
    let per_frame = CHIP_HZ / FRAME_HZ;
    let mut owed = 0.0f64;
    for f in 0..frames {
        for &(_, port, addr, v) in writes.iter().filter(|w| w.0 == f) {
            chip.write(port, addr, v);
        }
        owed += per_frame;
        let n = owed.floor();
        owed -= n;
        dac.run(&mut chip, n as u32);
    }
    chip.take_samples()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drums;

    const PIANO: Patch = Patch {
        name: "Test",
        algorithm: 4,
        feedback: 3,
        ops: [
            Op::new(2, 30, 31, 10, 2, 4, 6),
            Op::new(1, 0, 31, 6, 3, 2, 6),
            Op::new(1, 30, 31, 10, 2, 4, 6),
            Op::new(1, 4, 31, 6, 3, 2, 6),
        ],
        ams: 0,
        pms: 0,
    };

    #[test]
    fn notes_parse() {
        assert_eq!(parse_note("A4"), Some(69));
        assert_eq!(parse_note("Bb2"), Some(46));
        assert_eq!(note_name(61), "C#4");
    }

    #[test]
    fn a_song_renders_rows_levels_and_drums() {
        static INST: [Instrument; 1] = [Instrument::plain(PIANO)];
        static KIT: [Drum; 2] = [Drum { name: 'K', make: drums::kick }, Drum { name: 'H', make: drums::hat }];
        let song = Song {
            name: "t",
            speed: 6,
            rows: ["C4 . E4 - . .", "", "", "C3 . . . . .", "", "K . H . . ."],
            instruments: &INST,
            drums: &KIT,
            pan: [0xC0; 6],
            lfo: 0,
        };
        let r = render(&song, 6, 22050);
        assert_eq!(r.rows.len(), 6);
        assert_eq!(r.rows[0][0].label, "C4");
        assert_eq!(r.rows[2][0].label, "E4");
        assert_eq!(r.rows[0][3].label, "C3");
        assert_eq!(r.rows[0][5].label, "K");
        assert_eq!(r.rows[2][5].label, "H");
        assert!(r.rows[0][0].volume >= 12, "{}", r.rows[0][0].volume);
        // Keyed off on row 3: by row 5 it has faded.
        assert!(r.rows[5][0].volume < r.rows[2][0].volume, "{:?}", r.rows.iter().map(|c| c[0].volume).collect::<Vec<_>>());
        assert_eq!(r.rows[1][1].volume, 0);
        let expected = (6.0 * 6.0 / FRAME_HZ * 22050.0) as usize * 2;
        assert!((r.samples.len() as i64 - expected as i64).abs() < 100, "{} {}", r.samples.len(), expected);
        assert!(r.samples.iter().any(|s| s.abs() > 0.1));
    }
}
