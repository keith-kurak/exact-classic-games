//! The musicbox's songs and sound effects, all original, for the nes-apu
//! engine. Songs are tracker text for its driver; sound effects are register
//! writes, frame by frame, the way game code made them.

use nes_apu::driver::{Drum, Instrument, Song};
use nes_apu::{note_hz, pulse_period};

// ── Instruments ──
const LEAD: Instrument = Instrument { duty: 2, volume: &[13, 12, 11, 10, 10, 9], release: &[6, 4, 2, 1], arpeggio: &[], vibrato: (2, 8, 14), sweep: 0 };
const MAJOR: Instrument = Instrument { duty: 1, volume: &[7, 6, 6, 5], release: &[3, 2, 1], arpeggio: &[0, 0, 4, 4, 7, 7], vibrato: (0, 0, 0), sweep: 0 };
const MINOR: Instrument = Instrument { duty: 1, volume: &[7, 6, 6, 5], release: &[3, 2, 1], arpeggio: &[0, 0, 3, 3, 7, 7], vibrato: (0, 0, 0), sweep: 0 };
const BASS: Instrument = Instrument::plain(0, &[15], &[]);
const SAD_LEAD: Instrument = Instrument { duty: 1, volume: &[11, 11, 10, 10, 9, 9, 8], release: &[5, 3, 2, 1], arpeggio: &[], vibrato: (3, 10, 18), sweep: 0 };
const ECHO: Instrument = Instrument::plain(2, &[5, 5, 4, 4, 4, 3], &[2, 1]);
const STAB: Instrument = Instrument { duty: 0, volume: &[10, 8, 6, 5, 4, 3, 2], release: &[1], arpeggio: &[0, 7, 12], vibrato: (0, 0, 0), sweep: 0 };
const HERO: Instrument = Instrument { duty: 2, volume: &[14, 13, 12, 12, 11, 11, 10], release: &[6, 4, 2, 1], arpeggio: &[], vibrato: (2, 6, 10), sweep: 0 };

// ── Drum kits (noise period: 0 hiss … 15 rumble) ──
const KIT: [Drum; 3] = [
    Drum { name: 'K', period: 12, short: false, volume: &[15, 12, 8, 5, 2], periods: &[13, 14, 15] },
    Drum { name: 'S', period: 6, short: false, volume: &[13, 11, 9, 7, 5, 3, 1], periods: &[] },
    Drum { name: 'H', period: 1, short: false, volume: &[7, 4, 2], periods: &[] },
];

static MORNING_INST: [Instrument; 4] = [LEAD, MAJOR, MINOR, BASS];
static NOCTURNE_INST: [Instrument; 3] = [SAD_LEAD, ECHO, BASS];
static CLASH_INST: [Instrument; 3] = [HERO, STAB, BASS];
static DRUMS: [Drum; 3] = KIT;

/// Three songs, four bars of sixteen rows each, looping.
pub static SONGS: [Song; 3] = [
    Song {
        name: "Morning Road",
        speed: 6,
        rows: [
            "E5 . G5 . C6 . G5 . E5 . D5 . C5 . D5 . | E5 . . . A5 . G5 . E5 . C5 . A4 . . . |
             F5 . A5 . C6 . A5 . F5 . E5 . D5 . C5 . | D5 . E5 . G5 . . . B4 . D5 . G5 - . .",
            "@1C4 . . . . . . . . . . . . . . . | @2A3 . . . . . . . . . . . . . . . |
             @1F3 . . . . . . . . . . . . . . . | @1G3 . . . . . . . . . . . . . - .",
            "C3 . . C3 G2 . C3 . C3 . . C3 G2 . E2 . | A2 . . A2 E2 . A2 . A2 . . A2 E2 . G2 . |
             F2 . . F2 C3 . F2 . F2 . . F2 C3 . A2 . | G2 . . G2 D3 . G2 . G2 . . G2 D3 . B2 .",
            "K . H . S . H . K . K H S . H .",
        ],
        instruments: &MORNING_INST,
        drums: &DRUMS,
    },
    Song {
        name: "Castle Nocturne",
        speed: 7,
        rows: [
            "D5 . . . F5 . A5 . . . G5 . F5 . E5 . | D5 . . . . . Bb4 . D5 . F5 . . . E5 . |
             E5 . . . G5 . C6 . . . Bb5 . A5 . G5 . | A5 . . . . . . . C#5 . E5 . A5 - . .",
            "@1A4 . . . . . . . F4 . . . . . . . | @1F4 . . . . . . . D4 . . . . . . . |
             @1G4 . . . . . . . E4 . . . . . . . | @1E4 . . . . . . . C#4 . . . . . - .",
            "D3 . D3 . D3 . D3 . D3 . D3 . C3 . D3 . | Bb2 . Bb2 . Bb2 . Bb2 . Bb2 . Bb2 . A2 . Bb2 . |
             C3 . C3 . C3 . C3 . C3 . C3 . Bb2 . C3 . | A2 . A2 . A2 . A2 . A2 . A2 . G2 . A2 .",
            "K . . . S . . K K . . . S . H H",
        ],
        instruments: &NOCTURNE_INST,
        drums: &DRUMS,
    },
    Song {
        name: "Stance Clash",
        speed: 5,
        rows: [
            "A4 . C5 . E5 . A5 . G5 . E5 . C5 . E5 . | G5 . . . D5 . B4 . G4 . B4 . D5 . G5 . |
             F5 . E5 . F5 . A5 . C6 . A5 . F5 . C5 . | E5 . G#5 . B5 . E6 . D6 . B5 . G#5 . E5 .",
            "@1A3 . . A3 . . A3 . A3 . . A3 . . A3 . | @1G3 . . G3 . . G3 . G3 . . G3 . . G3 . |
             @1F3 . . F3 . . F3 . F3 . . F3 . . F3 . | @1E3 . . E3 . . E3 . E3 . . E3 . . E3 .",
            "A2 A3 A2 A3 A2 A3 A2 A3 A2 A3 A2 A3 A2 A3 A2 A3 | G2 G3 G2 G3 G2 G3 G2 G3 G2 G3 G2 G3 G2 G3 G2 G3 |
             F2 F3 F2 F3 F2 F3 F2 F3 F2 F3 F2 F3 F2 F3 F2 F3 | E2 E3 E2 E3 E2 E3 E2 E3 E2 E3 E2 E3 E2 E3 E2 E3",
            "K H S H K H S H K H S H K K S S",
        ],
        instruments: &CLASH_INST,
        drums: &DRUMS,
    },
];

/// A pulse note's three timer registers ($4002–$4003, or $4006–$4007) with a
/// length index, as writes at `frame`.
fn pulse_note(writes: &mut Vec<(u32, u16, u8)>, frame: u32, second: bool, midi: i32, length: u8) {
    let p = pulse_period(note_hz(midi));
    let base = if second { 0x4004 } else { 0x4000 };
    writes.push((frame, base + 2, (p & 0xFF) as u8));
    writes.push((frame, base + 3, ((p >> 8) as u8 & 7) | (length << 3)));
}

/// A sound effect: its name, file and register writes over some frames.
pub struct Effect {
    pub name: &'static str,
    pub file: &'static str,
    pub frames: u32,
    pub writes: Vec<(u32, u16, u8)>,
}

/// Seven sound effects, each showing a part of the hardware at work.
pub fn effects() -> Vec<Effect> {
    let mut out = Vec::new();
    // Jump: the sweep unit carries a square wave up, the length counter cuts it.
    let mut w = vec![(0, 0x4000, 0x9C), (0, 0x4001, 0xAA)];
    pulse_note(&mut w, 0, false, 57, 14);
    out.push(Effect { name: "Jump", file: "jump", frames: 24, writes: w });
    // Coin: two notes on the envelope's own decay; the second retriggers it.
    let mut w = vec![(0, 0x4004, 0x41), (0, 0x4005, 0x08)];
    pulse_note(&mut w, 0, true, 83, 1);
    pulse_note(&mut w, 5, true, 88, 1);
    out.push(Effect { name: "Coin", file: "coin", frames: 40, writes: w });
    // Laser: a thin 12.5% pulse swept down fast.
    let mut w = vec![(0, 0x4000, 0x1F), (0, 0x4001, 0x92)];
    pulse_note(&mut w, 0, false, 96, 12);
    out.push(Effect { name: "Laser", file: "laser", frames: 20, writes: w });
    // Explosion: long-mode noise from hiss to rumble, its envelope decaying slowly.
    let mut w = vec![(0, 0x400C, 0x0C), (0, 0x400E, 0x0A), (0, 0x400F, 0x08)];
    for (f, p) in [(4, 0x0B), (8, 0x0C), (14, 0x0D), (22, 0x0E), (32, 0x0F)] {
        w.push((f, 0x400E, p));
    }
    out.push(Effect { name: "Explosion", file: "explosion", frames: 90, writes: w });
    // Power up: an arpeggio climbing two octaves, volume set each frame.
    let mut w = vec![(0, 0x4001, 0x08)];
    for (i, n) in [72, 76, 79, 84, 88, 91, 96, 100].into_iter().enumerate() {
        let f = i as u32 * 3;
        w.push((f, 0x4000, 0xB0 | (15 - i as u8)));
        pulse_note(&mut w, f, false, n, 1);
    }
    w.push((26, 0x4000, 0xB0));
    out.push(Effect { name: "Power up", file: "powerup", frames: 34, writes: w });
    // Hit: a short burst of metallic (short-mode) noise over a falling blip.
    let mut w = vec![(0, 0x400C, 0x01), (0, 0x400E, 0x85), (0, 0x400F, 0x18), (0, 0x4000, 0x81), (0, 0x4001, 0x93)];
    pulse_note(&mut w, 0, false, 64, 1);
    out.push(Effect { name: "Hit", file: "hit", frames: 24, writes: w });
    // Fanfare: a little rising call on two pulses and the triangle together.
    let mut w = vec![(0, 0x4001, 0x08), (0, 0x4005, 0x08), (0, 0x4008, 0xFF)];
    for (i, (a, b, t)) in [(67, 64, 48), (71, 67, 55), (74, 71, 43), (79, 74, 55)].into_iter().enumerate() {
        let f = i as u32 * 7;
        w.push((f, 0x4000, 0xBC));
        w.push((f, 0x4004, 0x7A));
        pulse_note(&mut w, f, false, a, 1);
        pulse_note(&mut w, f, true, b, 1);
        let p = nes_apu::triangle_period(note_hz(t));
        w.push((f, 0x400A, (p & 0xFF) as u8));
        w.push((f, 0x400B, (p >> 8) as u8 | 0x08));
    }
    for f in 28..48u32 {
        let v = 12u8.saturating_sub(((f - 28) / 2) as u8);
        w.push((f, 0x4000, 0xB0 | v));
        w.push((f, 0x4004, 0x70 | v.saturating_sub(2)));
    }
    w.push((48, 0x4008, 0x80));
    out.push(Effect { name: "Fanfare", file: "fanfare", frames: 60, writes: w });
    out
}
