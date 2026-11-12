//! The soundscapes: three original songs and seven sound effects for the
//! ym2612 engine. Songs are tracker text and FM patches for its driver, with
//! DAC drums on channel 6; sound effects are register writes, frame by frame.
//!
//! An operator is `Op::new(multiple, total level, attack, decay, sustain
//! rate, sustain level, release)`; total level is 0 (loudest) to 127.

use ym2612::driver::{Drum, Instrument, Op, Patch, Script, Song};
use ym2612::{drums, fnum_for, note_hz};

// ── Patches ──

/// A plucked, growling bass: two stacks, the first with feedback.
const DRIVE_BASS: Patch = Patch {
    name: "DRIVE BASS",
    algorithm: 4,
    feedback: 5,
    ops: [
        Op::new(1, 26, 31, 10, 4, 2, 15),
        Op::new(1, 2, 31, 8, 3, 3, 11),
        Op::new(1, 24, 31, 14, 6, 5, 15),
        Op::new(1, 8, 31, 6, 3, 2, 11).dt(3),
    ],
    ams: 0,
    pms: 0,
};

/// A bright saw-like lead: feedback on 1 into 2, a second stack an octave
/// of brightness above, detuned a hair flat.
const SAW_LEAD: Patch = Patch {
    name: "SAW LEAD",
    algorithm: 4,
    feedback: 6,
    ops: [
        Op::new(1, 22, 28, 5, 2, 2, 7),
        Op::new(1, 4, 31, 4, 1, 1, 7),
        Op::new(2, 32, 28, 5, 2, 2, 7),
        Op::new(1, 10, 31, 4, 1, 1, 7).dt(7),
    ],
    ams: 0,
    pms: 0,
};

/// An electric piano: a fast-fading high "tine" over a warm body.
const E_PIANO: Patch = Patch {
    name: "E.PIANO",
    algorithm: 4,
    feedback: 0,
    ops: [
        Op::new(14, 50, 31, 18, 0, 15, 15),
        Op::new(1, 4, 31, 7, 4, 4, 8),
        Op::new(1, 32, 31, 9, 3, 5, 8),
        Op::new(1, 8, 31, 6, 4, 3, 8).dt(3),
    ],
    ams: 0,
    pms: 0,
};

/// A bell: inharmonic pairs (7 into 2, 5 into 1), ringing long.
const GLASS_BELL: Patch = Patch {
    name: "GLASS BELL",
    algorithm: 4,
    feedback: 0,
    ops: [
        Op::new(7, 36, 31, 8, 3, 4, 6),
        Op::new(2, 8, 31, 5, 3, 3, 5),
        Op::new(5, 40, 31, 10, 4, 6, 6).dt(3),
        Op::new(1, 6, 31, 4, 2, 3, 5),
    ],
    ams: 0,
    pms: 0,
};

/// A slow pad: soft attacks, two detuned stacks, LFO vibrato.
const DUSK_PAD: Patch = Patch {
    name: "DUSK PAD",
    algorithm: 4,
    feedback: 2,
    ops: [
        Op::new(1, 34, 16, 2, 0, 1, 5),
        Op::new(1, 10, 13, 2, 0, 1, 5).dt(2),
        Op::new(3, 44, 16, 2, 0, 1, 5),
        Op::new(2, 16, 13, 2, 0, 1, 5).dt(6),
    ],
    ams: 0,
    pms: 3,
};

/// A round, nearly pure sub bass.
const SUB_BASS: Patch = Patch {
    name: "SUB BASS",
    algorithm: 4,
    feedback: 0,
    ops: [
        Op::new(1, 40, 31, 6, 2, 3, 9),
        Op::new(1, 2, 31, 3, 1, 2, 8),
        Op::new(1, 127, 31, 0, 0, 0, 15),
        Op::new(1, 127, 31, 0, 0, 0, 15),
    ],
    ams: 0,
    pms: 0,
};

/// A gritty, overdriven lead: feedback into a three-times operator, mixed
/// with a plain one, through a carrier.
const OVERDRIVE: Patch = Patch {
    name: "OVERDRIVE",
    algorithm: 3,
    feedback: 6,
    ops: [
        Op::new(1, 20, 31, 6, 2, 2, 8),
        Op::new(3, 30, 31, 8, 3, 3, 8),
        Op::new(1, 26, 31, 6, 2, 2, 8),
        Op::new(1, 2, 31, 3, 1, 1, 8),
    ],
    ams: 0,
    pms: 0,
};

/// A brass-like stab: one modulator into three carriers.
const BRASS_STAB: Patch = Patch {
    name: "BRASS STAB",
    algorithm: 5,
    feedback: 5,
    ops: [
        Op::new(1, 24, 24, 8, 4, 3, 10),
        Op::new(1, 8, 31, 6, 3, 2, 9),
        Op::new(2, 14, 31, 7, 3, 3, 9).dt(2),
        Op::new(1, 10, 31, 6, 3, 2, 9).dt(6),
    ],
    ams: 0,
    pms: 0,
};

/// A palm-muted chug for power chords.
const CHUG: Patch = Patch {
    name: "CHUG GUITAR",
    algorithm: 3,
    feedback: 7,
    ops: [
        Op::new(1, 22, 31, 12, 6, 4, 13),
        Op::new(2, 30, 31, 12, 6, 4, 13),
        Op::new(1, 30, 31, 10, 6, 4, 13),
        Op::new(1, 4, 31, 9, 5, 3, 12),
    ],
    ams: 0,
    pms: 0,
};

/// Every patch, for the screen's list of names and algorithms.
pub const PATCHES: [Patch; 9] = [DRIVE_BASS, SAW_LEAD, E_PIANO, GLASS_BELL, DUSK_PAD, SUB_BASS, OVERDRIVE, BRASS_STAB, CHUG];

// ── Drums (channel 6's DAC) ──

static KIT: [Drum; 8] = [
    Drum { name: 'K', make: drums::kick },
    Drum { name: 'S', make: drums::snare },
    Drum { name: 'H', make: drums::hat },
    Drum { name: 'O', make: drums::open_hat },
    Drum { name: 'P', make: drums::clap },
    Drum { name: 'T', make: drums::high_tom },
    Drum { name: 'L', make: drums::low_tom },
    Drum { name: 'C', make: drums::crash },
];

// ── Songs ──
// Channels: 1 lead, 2 arpeggio or stabs, 3 and 5 chord tones, 4 bass, 6 drums.

static HIGHWAY_INST: [Instrument; 4] = [
    Instrument::plain(SAW_LEAD).vibrato(18.0, 12, 14),
    Instrument::plain(E_PIANO).volume(10),
    Instrument::plain(DUSK_PAD).volume(8),
    Instrument::plain(DRIVE_BASS),
];
static DEPTHS_INST: [Instrument; 4] = [
    Instrument::plain(GLASS_BELL).volume(2),
    Instrument::plain(E_PIANO).volume(6),
    Instrument::plain(DUSK_PAD).volume(4),
    Instrument::plain(SUB_BASS),
];
static RIDGE_INST: [Instrument; 4] = [
    Instrument::plain(OVERDRIVE).vibrato(14.0, 10, 10),
    Instrument::plain(BRASS_STAB).volume(4),
    Instrument::plain(CHUG).volume(6),
    Instrument::plain(DRIVE_BASS),
];

pub static SONGS: [Song; 3] = [
    Song {
        name: "Neon Highway",
        speed: 6,
        rows: [
            "@0B4 . . . E5 . . . G5 . F#5 . E5 . D5 . | E5 . . . . . . . C5 . D5 . E5 . G5 . |
             D5 . . . . . B4 . D5 . . . G5 . . . | F#5 . . . E5 . D5 . A4 . . . - . . . |
             B4 . . . E5 . . . G5 . A5 . B5 . . . | C6 . . . B5 . A5 . G5 . . . E5 . . . |
             A5 . . . G5 . E5 . C5 . . . D5 . E5 . | D#5 . . . . . . . F#5 . . . B4 . - .",
            "@1E4 . G4 . B4 . E5 . G4 . B4 . E5 . B4 . | C4 . E4 . G4 . C5 . E4 . G4 . C5 . G4 . |
             G3 . B3 . D4 . G4 . B3 . D4 . G4 . D4 . | D4 . F#4 . A4 . D5 . F#4 . A4 . D5 . A4 . |
             E4 . G4 . B4 . E5 . G4 . B4 . E5 . B4 . | C4 . E4 . G4 . C5 . E4 . G4 . C5 . G4 . |
             A3 . C4 . E4 . A4 . C4 . E4 . A4 . E4 . | B3 . D#4 . F#4 . B4 . D#4 . F#4 . B4 . F#4 .",
            "@2G4 . . . . . . . . . . . . . . . | E4 . . . . . . . . . . . . . . . |
             B3 . . . . . . . . . . . . . . . | F#4 . . . . . . . . . . . . . . . |
             G4 . . . . . . . . . . . . . . . | E4 . . . . . . . . . . . . . . . |
             C4 . . . . . . . . . . . . . . . | D#4 . . . . . . . . . . . . . - .",
            "@3E2 . E3 . E2 . E3 . E2 . E3 . E2 . E3 . | C2 . C3 . C2 . C3 . C2 . C3 . C2 . C3 . |
             G2 . G3 . G2 . G3 . G2 . G3 . G2 . G3 . | D2 . D3 . D2 . D3 . D2 . D3 . D2 . D3 . |
             E2 . E3 . E2 . E3 . E2 . E3 . E2 . E3 . | C2 . C3 . C2 . C3 . C2 . C3 . C2 . C3 . |
             A1 . A2 . A1 . A2 . A1 . A2 . A1 . A2 . | B1 . B2 . B1 . B2 . B1 . B2 . B1 . B2 .",
            "@2B4 . . . . . . . . . . . . . . . | G4 . . . . . . . . . . . . . . . |
             D4 . . . . . . . . . . . . . . . | A4 . . . . . . . . . . . . . . . |
             B4 . . . . . . . . . . . . . . . | G4 . . . . . . . . . . . . . . . |
             E4 . . . . . . . . . . . . . . . | F#4 . . . . . . . . . . . . . - .",
            "C . H . S . H . K . K . S . H H | K . H . S . H . K . K . S . H O |
             K . H . S . H . K . K . S . H H | K . H . S . H . K . K . S . H O |
             K . H . S . H . K . K . S . H H | K . H . S . H . K . K . S . H O |
             K . H . S . H . K . K . S . H H | K . S . K . S S T . T . L . L L",
        ],
        instruments: &HIGHWAY_INST,
        drums: &KIT,
        pan: [0xC0, 0x80, 0x40, 0xC0, 0x80, 0xC0],
        lfo: 0x08 | 3,
    },
    Song {
        name: "Coral Depths",
        speed: 8,
        rows: [
            "@0A5 . . . . . . . F5 . . . E5 . . . | D5 . . . . . . . . . . . - . . . |
             C5 . . . A4 . . . C5 . . . F5 . . . | E5 . . . . . . . G5 . . . - . . . |
             A5 . . . . . C6 . . . A5 . G5 . F5 . | F5 . . . . . . . D5 . . . - . . . |
             C5 . . . F5 . . . A5 . . . C6 . . . | G5 . . . . . . . E5 . . . - . . .",
            "@1D4 . . A4 . . E5 . F4 . . C5 . . A4 . | Bb3 . . F4 . . C5 . D4 . . A4 . . F4 . |
             F3 . . C4 . . G4 . A3 . . E4 . . C4 . | C4 . . G4 . . D5 . E4 . . B4 . . G4 .",
            "@2F4 . . . . . . . . . . . . . . . | D4 . . . . . . . . . . . . . . . |
             A4 . . . . . . . . . . . . . . . | E4 . . . . . . . . . . . . . . .",
            "@3D2 . . . . . . . . . . D2 . . A1 . | Bb1 . . . . . . . . . . Bb1 . . F2 . |
             F1 . . . . . . . . . . F1 . . C2 . | C2 . . . . . . . . . . C2 . . G1 .",
            "@2C5 . . . . . . . . . . . . . . . | A4 . . . . . . . . . . . . . . . |
             E5 . . . . . . . . . . . . . . . | G4 . . . . . . . . . . . . . . .",
            "K . . . . . H . P . . . . . H . | K . . . . . H . P . . K . . H H",
        ],
        instruments: &DEPTHS_INST,
        drums: &KIT,
        pan: [0xC0, 0x80, 0x40, 0xC0, 0x80, 0xC0],
        lfo: 0x08 | 2,
    },
    Song {
        name: "Turbo Ridge",
        speed: 5,
        rows: [
            "@0A4 . . . C5 . . . E5 . . . D5 . C5 . | C5 . . . . . A4 . F5 . . . E5 . D5 . |
             D5 . . . . . B4 . G5 . . . F5 . E5 . | E5 . . . . . . . G#5 . . . B5 . . . |
             A5 . . . G5 . E5 . A5 . . . C6 . B5 . | A5 . . . . . F5 . C6 . . . A5 . F5 . |
             F5 . . . E5 . D5 . A5 . . . F5 . D5 . | E5 . . . . . . . B4 . . . E5 . - .",
            "@1C5 . - . . . C5 - . . C5 . - . . . | A4 . - . . . A4 - . . A4 . - . . . |
             B4 . - . . . B4 - . . B4 . - . . . | G#4 . - . . . G#4 - . . G#4 . - . . . |
             C5 . - . . . C5 - . . C5 . - . . . | A4 . - . . . A4 - . . A4 . - . . . |
             F4 . - . . . F4 - . . F4 . - . . . | G#4 . - . . . G#4 - . . G#4 . - . . .",
            "@2E4 . E4 . E4 . E4 . E4 . E4 . E4 . E4 . | C4 . C4 . C4 . C4 . C4 . C4 . C4 . C4 . |
             D4 . D4 . D4 . D4 . D4 . D4 . D4 . D4 . | B3 . B3 . B3 . B3 . B3 . B3 . B3 . B3 . |
             E4 . E4 . E4 . E4 . E4 . E4 . E4 . E4 . | C4 . C4 . C4 . C4 . C4 . C4 . C4 . C4 . |
             A3 . A3 . A3 . A3 . A3 . A3 . A3 . A3 . | B3 . B3 . B3 . B3 . B3 . B3 . B3 . B3 .",
            "@3A1 . A2 A1 . A1 A2 . A1 . A2 A1 G2 . E2 . | F1 . F2 F1 . F1 F2 . F1 . F2 F1 E2 . C2 . |
             G1 . G2 G1 . G1 G2 . G1 . G2 G1 F2 . D2 . | E1 . E2 E1 . E1 E2 . E1 . E2 E1 D2 . B1 . |
             A1 . A2 A1 . A1 A2 . A1 . A2 A1 G2 . E2 . | F1 . F2 F1 . F1 F2 . F1 . F2 F1 E2 . C2 . |
             D2 . D3 D2 . D2 D3 . D2 . D3 D2 C3 . A2 . | E1 . E2 E1 . E1 E2 . E1 . E2 E1 D2 . B1 .",
            "@2A3 . A3 . A3 . A3 . A3 . A3 . A3 . A3 . | F3 . F3 . F3 . F3 . F3 . F3 . F3 . F3 . |
             G3 . G3 . G3 . G3 . G3 . G3 . G3 . G3 . | E3 . E3 . E3 . E3 . E3 . E3 . E3 . E3 . |
             A3 . A3 . A3 . A3 . A3 . A3 . A3 . A3 . | F3 . F3 . F3 . F3 . F3 . F3 . F3 . F3 . |
             D3 . D3 . D3 . D3 . D3 . D3 . D3 . D3 . | E3 . E3 . E3 . E3 . E3 . E3 . E3 . E3 .",
            "C . H . S . H K K . H . S . H . | K . H . S . H K K . H . S . H . |
             K . H . S . H K K . H . S . H . | K . H . S . H K K . H . S . H O |
             K . H . S . H K K . H . S . H . | K . H . S . H K K . H . S . H . |
             K . H . S . H K K . H . S . H . | S . S . S S T . T . L L S S S S",
        ],
        instruments: &RIDGE_INST,
        drums: &KIT,
        pan: [0xC0, 0x40, 0x80, 0xC0, 0x40, 0xC0],
        lfo: 0,
    },
];

/// Which patch (in `PATCHES`) an instrument plays.
pub fn patch_index(p: &Patch) -> usize {
    PATCHES.iter().position(|q| q.name == p.name).unwrap_or(0)
}

/// A sound effect: its name, file, register writes, length in frames, and a
/// sample streamed to the DAC from its start (or none).
pub struct Effect {
    pub name: &'static str,
    pub file: &'static str,
    pub frames: u32,
    pub script: Script,
    pub dac: Vec<u8>,
}

fn inst(p: Patch) -> Instrument {
    Instrument::plain(p)
}

/// A plain sine patch on operator 4 alone (algorithm 7).
const SINE: Patch = Patch {
    name: "SINE",
    algorithm: 7,
    feedback: 0,
    ops: [
        Op::new(1, 127, 31, 0, 0, 0, 15),
        Op::new(1, 127, 31, 0, 0, 0, 15),
        Op::new(1, 127, 31, 0, 0, 0, 15),
        Op::new(1, 0, 31, 0, 0, 0, 6),
    ],
    ams: 0,
    pms: 0,
};

/// Seven sound effects, each showing a part of the chip at work.
pub fn effects() -> Vec<Effect> {
    let mut out = Vec::new();

    // Chime: the bell patch, two notes a fifth apart.
    let mut s = Script::default();
    s.load(0, &inst(GLASS_BELL), 0xC0).note(0, 84);
    s.at(6).key(0, false).note(0, 91);
    s.at(14).key(0, false);
    out.push(Effect { name: "Chime", file: "chime", frames: 80, script: s, dac: vec![] });

    // Laser: one operator in full feedback (a buzzy saw), its pitch thrown
    // down two octaves a frame at a time.
    let mut s = Script::default();
    let zap = Patch { name: "ZAP", feedback: 7, ops: [Op::new(1, 0, 31, 4, 0, 0, 12), SINE.ops[1], SINE.ops[2], SINE.ops[3]], ..SINE };
    s.load(0, &inst(zap), 0xC0).write(0, 0x4C, 127);
    for f in 0..18u32 {
        let (b, n) = fnum_for(note_hz(96 - f as i32 * 2));
        s.at(f).pitch(0, b, n);
        if f == 0 {
            s.key(0, true);
        }
    }
    s.at(18).key(0, false);
    out.push(Effect { name: "Laser", file: "laser", frames: 30, script: s, dac: vec![] });

    // Explosion: a chain of four operators, deep feedback and low pitch:
    // FM noise, sinking and fading.
    let mut s = Script::default();
    let blast = Patch {
        name: "BLAST",
        algorithm: 0,
        feedback: 7,
        ops: [Op::new(0, 0, 31, 0, 0, 0, 4), Op::new(5, 6, 31, 2, 2, 2, 4), Op::new(3, 10, 31, 3, 2, 2, 4), Op::new(1, 0, 31, 4, 3, 2, 3)],
        ams: 0,
        pms: 0,
    };
    s.load(0, &inst(blast), 0xC0).note(0, 40);
    for f in 1..40u32 {
        let (b, n) = fnum_for(note_hz(40.0_f64.max(0.0) as i32 - (f as i32) / 2));
        s.at(f).pitch(0, b, n);
    }
    s.at(30).key(0, false);
    out.push(Effect { name: "Explosion", file: "explosion", frames: 90, script: s, dac: vec![] });

    // Warp: channel 3's special mode, each operator its own pitch, all four
    // heard (algorithm 7), climbing apart.
    let mut s = Script::default();
    let quad = Patch {
        name: "QUAD",
        algorithm: 7,
        feedback: 3,
        ops: [Op::new(1, 6, 31, 2, 1, 2, 7); 4],
        ams: 0,
        pms: 0,
    };
    s.load(2, &inst(quad), 0xC0).write(0, 0x27, 0x40);
    for f in 0..36u32 {
        s.at(f);
        // $A9/$AD operator 1, $AA/$AE operator 2, $A8/$AC operator 3, $A2/$A6 operator 4.
        for (k, (lo, hi)) in [(0xA9u8, 0xADu8), (0xAA, 0xAE), (0xA8, 0xAC), (0xA2, 0xA6)].into_iter().enumerate() {
            let midi = 55 + k as i32 * 4 + (f as i32 * (k as i32 + 1)) / 3;
            let (b, n) = fnum_for(note_hz(midi));
            s.write(0, hi, (b << 3) | (n >> 8) as u8).write(0, lo, n as u8);
        }
        if f == 0 {
            s.key(2, true);
        }
    }
    s.at(36).key(2, false);
    s.at(70).write(0, 0x27, 0);
    out.push(Effect { name: "Warp", file: "warp", frames: 70, script: s, dac: vec![] });

    // Siren: the LFO's deepest vibrato on a plain tone.
    let mut s = Script::default();
    let siren = Patch { pms: 7, ..SINE };
    s.write(0, 0x22, 0x08 | 1).load(0, &inst(siren), 0xC0).note(0, 79);
    s.at(84).key(0, false);
    s.at(110).write(0, 0x22, 0);
    out.push(Effect { name: "Siren", file: "siren", frames: 110, script: s, dac: vec![] });

    // Engine: SSG-EG repeating a fast decay: a pulsing, motor-like buzz.
    let mut s = Script::default();
    let motor = Patch {
        name: "MOTOR",
        algorithm: 4,
        feedback: 6,
        ops: [Op::new(1, 18, 31, 0, 0, 0, 15), Op::new(1, 0, 31, 22, 0, 15, 8).ssg(8), SINE.ops[2], SINE.ops[3]],
        ams: 0,
        pms: 0,
    };
    s.load(0, &inst(motor), 0xC0).note(0, 36);
    for f in 1..70u32 {
        let (b, n) = fnum_for(note_hz(36.0 as i32) * (1.0 + f as f64 * 0.012));
        s.at(f).pitch(0, b, n);
    }
    s.at(70).key(0, false);
    out.push(Effect { name: "Engine", file: "engine", frames: 84, script: s, dac: vec![] });

    // Boom: a sample through channel 6's DAC.
    let mut s = Script::default();
    s.write(0, 0x2B, 0x80).write(1, 0xB6, 0xC0);
    out.push(Effect { name: "DAC Boom", file: "boom", frames: 48, script: s, dac: drums::boom() });

    out
}
