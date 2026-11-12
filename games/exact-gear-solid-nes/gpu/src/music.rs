//! The game's music and sounds, made by the NES sound chip (engines/nes-apu)
//! and rendered to WAV by the `render` test. All original.
//!
//! "Infiltration": slow and tense, E Phrygian at about 90 beats a minute: a
//! triangle heartbeat, a thin 12.5% pulse turning over E, B and the flat
//! second (F), soft noise ticks, and a sparse lead with a slow vibrato that
//! waits two bars to come in.

use nes_apu::driver::{Drum, Instrument, Song};
use nes_apu::{note_hz, pulse_period, triangle_period};

const LEAD: Instrument = Instrument {
    duty: 1,
    volume: &[3, 5, 7, 8, 9, 9, 9, 8, 8, 8, 8, 7, 7, 7, 7, 6],
    release: &[5, 4, 3, 2, 1],
    arpeggio: &[],
    vibrato: (2, 14, 24),
    sweep: 0,
};
const PAD: Instrument = Instrument::plain(0, &[4, 5, 5, 4, 4, 4, 3], &[2, 1]);
const BASS: Instrument = Instrument::plain(0, &[15], &[]);

const KIT: [Drum; 3] = [
    Drum { name: 'H', period: 0, short: false, volume: &[3, 2, 1], periods: &[] },
    Drum { name: 'T', period: 13, short: false, volume: &[6, 5, 3, 2, 1], periods: &[14, 15] },
    Drum { name: 'S', period: 5, short: true, volume: &[3, 2, 1], periods: &[] },
];

static INSTRUMENTS: [Instrument; 3] = [LEAD, PAD, BASS];
static DRUMS: [Drum; 3] = KIT;

pub static INFILTRATION: Song = Song {
    name: "Infiltration",
    speed: 10,
    rows: [
        // The lead waits two bars, then hangs on the flat second.
        ". . . . . . . . . . . . . . . . | . . . . . . . . . . . . . . . . |
         B4 . . . . . . . C5 . . . . . . . | B4 . . . . . . . . . . . - . . . |
         E5 . . . . . . . F5 . . . E5 . . . | D5 . . . . . . . B4 . . . . . . . |
         C5 . . . . . . . B4 . . . A4 . . . | B4 . . . . . . . . . . . - . . .",
        "@1E4 . B3 . G4 . B3 . F4 . B3 . E4 . B3 . | E4 . B3 . G4 . B3 . F4 . B3 . E4 . B3 . |
         E4 . C4 . G4 . C4 . F4 . C4 . E4 . C4 . | E4 . B3 . G4 . B3 . F4 . B3 . E4 . B3 . |
         E4 . B3 . G4 . B3 . F4 . B3 . E4 . B3 . | E4 . B3 . G4 . B3 . F4 . B3 . E4 . B3 . |
         E4 . C4 . G4 . C4 . F4 . C4 . E4 . C4 . | F4 . C4 . A4 . C4 . G4 . C4 . F4 . C4 .",
        "@2E2 . . . - . . . E2 . . E2 - . . . | E2 . . . - . . . E2 . . E2 - . . . |
         C2 . . . - . . . C2 . . C2 - . . . | E2 . . . - . . . E2 . . E2 - . . . |
         E2 . . . - . . . E2 . . E2 - . . . | E2 . . . - . . . E2 . . E2 - . . . |
         C2 . . . - . . . C2 . . C2 - . . . | F2 . . . - . . . F2 . . . E2 . . .",
        "T . . . H . . . T . . . H . . S",
    ],
    instruments: &INSTRUMENTS,
    drums: &DRUMS,
};

fn pulse_note(w: &mut Vec<(u32, u16, u8)>, frame: u32, second: bool, midi: i32, length: u8) {
    let p = pulse_period(note_hz(midi));
    let base = if second { 0x4004 } else { 0x4000 };
    w.push((frame, base + 2, (p & 0xFF) as u8));
    w.push((frame, base + 3, ((p >> 8) as u8 & 7) | (length << 3)));
}

/// A sound effect: its file name, frames, and register writes.
pub struct Effect {
    pub file: &'static str,
    pub frames: u32,
    pub writes: Vec<(u32, u16, u8)>,
}

pub fn effects() -> Vec<Effect> {
    let mut out = Vec::new();
    // "!": a sharp rising sting on both pulses, over a crash of noise.
    let mut w = vec![(0, 0x4001, 0x08), (0, 0x4005, 0x08), (0, 0x400E, 0x03), (0, 0x400F, 0x08)];
    for (f, a, b) in [(0, 84, 79), (4, 91, 86)] {
        w.push((f, 0x4000, 0xBF));
        w.push((f, 0x4004, 0x7C));
        pulse_note(&mut w, f, false, a, 1);
        pulse_note(&mut w, f, true, b, 1);
    }
    for f in 0..30u32 {
        let v = 15u8.saturating_sub((f / 2) as u8);
        w.push((f, 0x400C, 0x30 | v.min(9)));
        if f >= 8 {
            w.push((f, 0x4000, 0xB0 | v));
            w.push((f, 0x4004, 0x70 | v.saturating_sub(3)));
        }
    }
    w.push((32, 0x4000, 0xB0));
    w.push((32, 0x4004, 0x70));
    w.push((32, 0x400C, 0x30));
    out.push(Effect { file: "alert", frames: 50, writes: w });
    // The box: a short rustle of noise sliding up in pitch.
    let mut w = vec![(0, 0x400F, 0x08)];
    for (f, p, v) in [(0, 9, 6), (2, 8, 7), (4, 7, 6), (6, 6, 5), (8, 5, 3), (10, 5, 2), (12, 5, 0)] {
        w.push((f, 0x400E, p));
        w.push((f, 0x400C, 0x30 | v));
    }
    out.push(Effect { file: "box", frames: 18, writes: w });
    // Mission complete: an E minor arpeggio rising to a held major E.
    let mut w = vec![(0, 0x4001, 0x08), (0, 0x4005, 0x08), (0, 0x4008, 0xFF)];
    for (i, n) in [64, 67, 71, 76, 79, 83].into_iter().enumerate() {
        let f = i as u32 * 5;
        w.push((f, 0x4000, 0xBC));
        pulse_note(&mut w, f, false, n, 1);
    }
    w.push((30, 0x4000, 0xBC));
    pulse_note(&mut w, 30, false, 80, 1);
    w.push((30, 0x4004, 0x7A));
    pulse_note(&mut w, 30, true, 76, 1);
    let t = triangle_period(note_hz(52));
    w.push((0, 0x400A, (t & 0xFF) as u8));
    w.push((0, 0x400B, (t >> 8) as u8 | 0x08));
    for f in 31..80u32 {
        let v = 12u8.saturating_sub(((f - 31) / 4) as u8);
        w.push((f, 0x4000, 0xB0 | v));
        w.push((f, 0x4004, 0x70 | v.saturating_sub(2)));
    }
    w.push((80, 0x4008, 0x80));
    out.push(Effect { file: "clear", frames: 90, writes: w });
    out
}
