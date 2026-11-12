//! The stage's music and sound effects, for the ym2612 engine: an original
//! song in tracker text with DAC drums, and effects as register writes,
//! frame by frame. The `render` test writes them to assets/ as WAV.

use ym2612::driver::{Drum, Instrument, Op, Patch, Script, Song};
use ym2612::{drums, fnum_for, note_hz};

/// A bright, reedy lead: a feedback stack into a carrier, plus a second
/// stack a fifth's brightness above.
const MESA_LEAD: Patch = Patch {
    name: "MESA LEAD",
    algorithm: 4,
    feedback: 5,
    ops: [
        Op::new(1, 24, 31, 6, 2, 2, 7),
        Op::new(1, 6, 31, 5, 1, 1, 7),
        Op::new(3, 38, 31, 8, 3, 3, 7),
        Op::new(1, 12, 31, 5, 1, 1, 7).dt(3),
    ],
    ams: 0,
    pms: 0,
};

/// A marimba-like pluck: a high, fast-fading modulator on a short body.
const PLUCK: Patch = Patch {
    name: "PLUCK",
    algorithm: 4,
    feedback: 0,
    ops: [
        Op::new(4, 34, 31, 16, 0, 15, 15),
        Op::new(1, 6, 31, 10, 6, 6, 10),
        Op::new(1, 36, 31, 12, 4, 6, 10),
        Op::new(1, 12, 31, 9, 6, 5, 10).dt(2),
    ],
    ams: 0,
    pms: 0,
};

/// A soft pad with vibrato from the LFO.
const SUNSET_PAD: Patch = Patch {
    name: "SUNSET PAD",
    algorithm: 4,
    feedback: 2,
    ops: [
        Op::new(1, 36, 18, 2, 0, 1, 6),
        Op::new(1, 12, 14, 2, 0, 1, 6).dt(2),
        Op::new(2, 44, 18, 2, 0, 1, 6),
        Op::new(1, 16, 14, 2, 0, 1, 6).dt(6),
    ],
    ams: 0,
    pms: 2,
};

/// A slap bass: feedback-growled, quick to fade.
const SLAP_BASS: Patch = Patch {
    name: "SLAP BASS",
    algorithm: 4,
    feedback: 6,
    ops: [
        Op::new(1, 24, 31, 12, 5, 3, 15),
        Op::new(1, 2, 31, 9, 4, 3, 11),
        Op::new(2, 30, 31, 14, 7, 5, 15),
        Op::new(1, 8, 31, 7, 4, 2, 11).dt(3),
    ],
    ams: 0,
    pms: 0,
};

/// A bell for effects: inharmonic pairs, ringing.
const BELL: Patch = Patch {
    name: "BELL",
    algorithm: 4,
    feedback: 0,
    ops: [
        Op::new(7, 34, 31, 8, 3, 4, 7),
        Op::new(2, 4, 31, 6, 4, 4, 6),
        Op::new(5, 40, 31, 10, 4, 6, 7).dt(3),
        Op::new(1, 6, 31, 5, 3, 4, 6),
    ],
    ams: 0,
    pms: 0,
};

/// A brassy fanfare voice: one modulator into three carriers.
const BRASS: Patch = Patch {
    name: "BRASS",
    algorithm: 5,
    feedback: 5,
    ops: [
        Op::new(1, 22, 22, 6, 3, 3, 9),
        Op::new(1, 8, 31, 5, 2, 2, 8),
        Op::new(2, 14, 31, 6, 2, 3, 8).dt(2),
        Op::new(1, 10, 31, 5, 2, 2, 8).dt(6),
    ],
    ams: 0,
    pms: 0,
};

/// A plain sine on operator 4 alone.
const SINE: Patch = Patch {
    name: "SINE",
    algorithm: 7,
    feedback: 0,
    ops: [
        Op::new(1, 127, 31, 0, 0, 0, 15),
        Op::new(1, 127, 31, 0, 0, 0, 15),
        Op::new(1, 127, 31, 0, 0, 0, 15),
        Op::new(1, 0, 31, 0, 0, 0, 8),
    ],
    ams: 0,
    pms: 0,
};

static KIT: [Drum; 7] = [
    Drum { name: 'K', make: drums::kick },
    Drum { name: 'S', make: drums::snare },
    Drum { name: 'H', make: drums::hat },
    Drum { name: 'O', make: drums::open_hat },
    Drum { name: 'P', make: drums::clap },
    Drum { name: 'T', make: drums::high_tom },
    Drum { name: 'L', make: drums::low_tom },
];

static STAGE_INST: [Instrument; 4] = [
    Instrument::plain(MESA_LEAD).vibrato(16.0, 10, 12),
    Instrument::plain(PLUCK).volume(8),
    Instrument::plain(SUNSET_PAD).volume(10),
    Instrument::plain(SLAP_BASS),
];

/// The stage: "Beetle Rush", G major, at a run. Channels: 1 lead, 2 plucked
/// arpeggios, 3 and 5 pad chord tones, 4 bass, 6 drums.
pub static STAGE: Song = Song {
    name: "Beetle Rush",
    speed: 5,
    rows: [
        "@0D5 . G5 . B5 . A5 G5 . . D5 . E5 . G5 . | B5 . . . A5 . G5 . E5 . . . - . . . |
         E5 . G5 . C6 . B5 A5 . . G5 . E5 . C5 . | D5 . . . F#5 . A5 . D6 . . . C6 . A5 . |
         B5 . . . A5 . B5 . D6 . B5 . A5 . G5 . | F#5 . . . D5 . F#5 . B5 . . . A5 . F#5 . |
         G5 . E5 . C5 . E5 . G5 . C6 . E6 . D6 . | D6 . . . . . . . A5 . . . - . . .",
        "@1G4 . B4 . D5 . B4 . G4 . B4 . D5 . B4 . | E4 . G4 . B4 . G4 . E4 . G4 . B4 . G4 . |
         C4 . E4 . G4 . E4 . C4 . E4 . G4 . E4 . | D4 . F#4 . A4 . F#4 . D4 . F#4 . A4 . F#4 . |
         G4 . B4 . D5 . B4 . G4 . B4 . D5 . B4 . | B3 . D4 . F#4 . D4 . B3 . D4 . F#4 . D4 . |
         C4 . E4 . G4 . E4 . C4 . E4 . G4 . E4 . | D4 . F#4 . A4 . F#4 . D4 . F#4 . A4 . F#4 .",
        "@2B4 . . . . . . . . . . . . . . . | G4 . . . . . . . . . . . . . . . |
         G4 . . . . . . . . . . . . . . . | A4 . . . . . . . . . . . . . . . |
         B4 . . . . . . . . . . . . . . . | B4 . . . . . . . . . . . . . . . |
         G4 . . . . . . . . . . . . . . . | A4 . . . . . . . . . . . . . . .",
        "@3G2 . G3 . G2 . G3 . G2 . G3 . G2 . G3 . | E2 . E3 . E2 . E3 . E2 . E3 . E2 . E3 . |
         C2 . C3 . C2 . C3 . C2 . C3 . C2 . C3 . | D2 . D3 . D2 . D3 . D2 . D3 . D2 . D3 . |
         G2 . G3 . G2 . G3 . G2 . G3 . G2 . G3 . | B1 . B2 . B1 . B2 . B1 . B2 . B1 . B2 . |
         C2 . C3 . C2 . C3 . C2 . C3 . C2 . C3 . | D2 . D3 . D2 . D3 . D2 . D3 . D2 . D3 .",
        "@2D4 . . . . . . . . . . . . . . . | B3 . . . . . . . . . . . . . . . |
         E4 . . . . . . . . . . . . . . . | F#4 . . . . . . . . . . . . . . . |
         D4 . . . . . . . . . . . . . . . | D4 . . . . . . . . . . . . . . . |
         E4 . . . . . . . . . . . . . . . | F#4 . . . . . . . . . . . . . . .",
        "K . H . S . H K K . H . S . H H | K . H . S . H K K . H . S . H O |
         K . H . S . H K K . H . S . H H | K . H . S . H K K . H . S P H O |
         K . H . S . H K K . H . S . H H | K . H . S . H K K . H . S . H O |
         K . H . S . H K K . H . S . H H | K . S . K . S S T . T . L . L L",
    ],
    instruments: &STAGE_INST,
    drums: &KIT,
    pan: [0xC0, 0x80, 0x40, 0xC0, 0x80, 0xC0],
    lfo: 0x08 | 3,
};

/// A sound effect: its file name, register writes and length in frames.
pub struct Effect {
    pub file: &'static str,
    pub frames: u32,
    pub script: Script,
}

fn inst(p: Patch) -> Instrument {
    Instrument::plain(p)
}

pub fn effects() -> Vec<Effect> {
    let mut out = Vec::new();

    // A beetle: two quick bell notes up a fourth.
    let mut s = Script::default();
    s.load(0, &inst(BELL), 0xC0).note(0, 88);
    s.at(3).key(0, false).note(0, 93);
    s.at(8).key(0, false);
    out.push(Effect { file: "beetle", frames: 40, script: s });

    // Jump: a sine swept up an octave as it rolls up.
    let mut s = Script::default();
    s.load(0, &inst(SINE), 0xC0);
    for f in 0..10u32 {
        let (b, n) = fnum_for(note_hz(64 + f as i32 * 2));
        s.at(f).pitch(0, b, n);
        if f == 0 {
            s.key(0, true);
        }
    }
    s.at(10).key(0, false);
    out.push(Effect { file: "jump", frames: 22, script: s });

    // A thorn: a buzzy saw in full feedback, thrown down two octaves, over
    // a bell scatter of falling beetles.
    let mut s = Script::default();
    let zap = Patch { name: "ZAP", feedback: 7, ops: [Op::new(1, 0, 31, 4, 0, 0, 12), SINE.ops[1], SINE.ops[2], SINE.ops[3]], ..SINE };
    s.load(0, &inst(zap), 0xC0).write(0, 0x4C, 127).load(1, &inst(BELL), 0xC0);
    for f in 0..20u32 {
        let (b, n) = fnum_for(note_hz(84 - f as i32 * 2));
        s.at(f).pitch(0, b, n);
        if f == 0 {
            s.key(0, true);
        }
        if f % 4 == 0 {
            s.key(1, false).note(1, 96 - f as i32);
        }
    }
    s.at(20).key(0, false).key(1, false);
    out.push(Effect { file: "thorn", frames: 50, script: s });

    // A checkpoint passed: a bright arpeggio on two bells.
    let mut s = Script::default();
    s.load(0, &inst(BELL), 0x80).load(1, &inst(BELL), 0x40);
    for (k, n) in [79, 83, 86, 91].into_iter().enumerate() {
        let f = k as u32 * 5;
        s.at(f).key(0, false).note(0, n).key(1, false).note(1, n + 12);
    }
    s.at(34).key(0, false).key(1, false);
    out.push(Effect { file: "checkpoint", frames: 80, script: s });

    // Not enough beetles: brass falling a tritone and sagging.
    let mut s = Script::default();
    s.load(0, &inst(BRASS), 0xC0).note(0, 67);
    s.at(12).key(0, false).note(0, 66);
    s.at(24).key(0, false).note(0, 61);
    for f in 30..70u32 {
        let (b, n) = fnum_for(note_hz(61) * (1.0 - (f - 30) as f64 * 0.002));
        s.at(f).pitch(0, b, n);
    }
    s.at(70).key(0, false);
    out.push(Effect { file: "fail", frames: 100, script: s });

    // Ready: a short rising call on brass over a drum-roll of bass hits,
    // before the music starts.
    let mut s = Script::default();
    s.load(0, &inst(BRASS), 0xC0).load(1, &inst(BRASS), 0x80).load(2, &inst(BELL), 0x40).load(3, &inst(SLAP_BASS), 0xC0);
    let call: [(u32, i32, i32); 6] = [(0, 67, 62), (6, 67, 62), (12, 71, 67), (18, 74, 71), (30, 72, 69), (36, 74, 71)];
    for (f, hi, lo) in call {
        s.at(f).key(0, false).note(0, hi).key(1, false).note(1, lo).key(3, false).note(3, 43 + (f as i32 / 12));
        s.at(f + 4).key(3, false);
        if f < 30 {
            s.at(f + 5).key(0, false).key(1, false);
        }
    }
    s.at(48).key(0, false).key(1, false).note(0, 79).note(1, 74).note(2, 91).note(3, 43);
    s.at(52).key(3, false);
    s.at(84).key(0, false).key(1, false).key(2, false);
    out.push(Effect { file: "ready", frames: 120, script: s });

    // The stage cleared: a fanfare in G on brass, chords on two channels.
    let mut s = Script::default();
    s.load(0, &inst(BRASS), 0xC0).load(1, &inst(BRASS), 0x80).load(2, &inst(BRASS), 0x40).load(3, &inst(SLAP_BASS), 0xC0);
    let hits: [(u32, [i32; 4]); 5] = [(0, [67, 71, 74, 43]), (8, [67, 71, 74, 43]), (16, [69, 72, 76, 45]), (24, [71, 74, 79, 47]), (36, [74, 79, 83, 43])];
    for (f, notes) in hits {
        s.at(f);
        for (ch, n) in notes.into_iter().enumerate() {
            s.key(ch, false).note(ch, n);
        }
        s.at(f + 6);
        if f < 36 {
            for ch in 0..4 {
                s.key(ch, false);
            }
        }
    }
    s.at(90);
    for ch in 0..4 {
        s.key(ch, false);
    }
    out.push(Effect { file: "clear", frames: 150, script: s });

    out
}
