//! The Yamaha YM2612 (OPN2), the Mega Drive's FM synthesis chip, emulated
//! sample by sample at its own rate (the NTSC console's 7.67 MHz master clock
//! over 144: about 53 267 samples a second) and driven the way a sound driver
//! drives it: by writing its registers.
//!
//! - Six channels of four operators. An operator is a sine wave read through
//!   a log-sine table and an exponent table (as the chip does, in integer
//!   steps), its pitch from the channel's F-number and block, its detune and
//!   multiple, its loudness from an envelope and total level.
//! - Eight algorithms connect each channel's operators; operator 1 can feed
//!   back into itself.
//! - The envelope generator: attack, first decay, sustain level, second
//!   decay and release, with rate key scaling, ticking every third sample
//!   through the chip's increment patterns; SSG-EG (repeat, alternate, hold,
//!   invert).
//! - The LFO: amplitude (tremolo) and pitch (vibrato) modulation, set per
//!   channel and enabled per operator.
//! - Channel 3's special mode (a frequency for each operator), and channel
//!   6's DAC (8-bit samples written to `$2A`).
//! - The output: each channel's sum clipped to 14 bits, cut to the 9-bit DAC,
//!   panned left, right or both, and resampled to the output rate.
//!
//! Not emulated: the timers and status flags (there is no CPU to read them),
//! CSM key-on, and the exact pipeline delays between operators.

pub mod driver;
pub mod drums;
pub mod wav;

use std::sync::OnceLock;

/// The NTSC Mega Drive's master clock, and the chip's sample rate from it.
pub const MASTER_HZ: f64 = 7_670_453.0;
pub const CHIP_HZ: f64 = MASTER_HZ / 144.0;

// ── Tables ──

struct Tables {
    /// −log2(sin) of a quarter wave, 256 steps, in 1/256ths (4.8 fixed point).
    logsin: [u16; 256],
    /// 2^(−x) of the fraction, as the chip's 10-bit exponent table.
    exp: [u16; 256],
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut logsin = [0u16; 256];
        let mut exp = [0u16; 256];
        for i in 0..256 {
            let s = ((i as f64 * 2.0 + 1.0) * std::f64::consts::PI / 1024.0).sin();
            logsin[i] = (-s.log2() * 256.0).round() as u16;
            exp[i] = ((2f64.powf((255 - i) as f64 / 256.0) - 1.0) * 1024.0).round() as u16;
        }
        Tables { logsin, exp }
    })
}

/// Detune, in phase steps, by detune setting (1–3) and key code (0–31).
const DETUNE: [[u8; 32]; 4] = [
    [0; 32],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 8, 8],
    [1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 16, 16, 16],
    [2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 20, 22, 22, 22, 22],
];

/// The key code's low two bits from the F-number's top four.
const KEY_FRACTION: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 3, 3, 3, 3, 3, 3];

/// Envelope increments: for each pattern, the step on each of eight ticks.
const EG_INC: [[u8; 8]; 18] = [
    [0, 1, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 1, 1, 1, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 1, 2, 1, 1, 1, 2],
    [1, 2, 1, 2, 1, 2, 1, 2],
    [1, 2, 2, 2, 1, 2, 2, 2],
    [2, 2, 2, 2, 2, 2, 2, 2],
    [2, 2, 2, 4, 2, 2, 2, 4],
    [2, 4, 2, 4, 2, 4, 2, 4],
    [2, 4, 4, 4, 2, 4, 4, 4],
    [4, 4, 4, 4, 4, 4, 4, 4],
    [4, 4, 4, 8, 4, 4, 4, 8],
    [4, 8, 4, 8, 4, 8, 4, 8],
    [4, 8, 8, 8, 4, 8, 8, 8],
    [8, 8, 8, 8, 8, 8, 8, 8],
    [0, 0, 0, 0, 0, 0, 0, 0],
];

/// An effective rate (0–63) as the ticks it waits (a shift) and its pattern.
fn rate_step(rate: u32) -> (u32, usize) {
    match rate {
        0 | 1 => (0, 17),
        r if r < 48 => (11 - r / 4, (r % 4) as usize),
        r if r < 60 => (0, 4 + (r - 48) as usize),
        _ => (0, 16),
    }
}

/// LFO: samples a step (of 128) for each frequency setting, about 3.98 Hz to
/// 72.2 Hz at the chip's rate.
const LFO_PERIOD: [u32; 8] = [108, 77, 71, 67, 62, 44, 8, 5];
/// Tremolo depth by AMS: how far the LFO's 0–126 is shifted down.
const AM_SHIFT: [u32; 4] = [8, 3, 1, 0];
/// Vibrato depth by PMS, in cents either way.
const PM_CENTS: [f32; 8] = [0.0, 3.4, 6.7, 10.0, 14.0, 20.0, 40.0, 80.0];

/// The DAC's crossover step (the "ladder effect"): non-negative outputs sit a
/// little higher, negative a little lower, in 9-bit units. An approximation.
const LADDER: i32 = 4;

// ── Operators and channels ──

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Stage {
    Attack,
    Decay,
    Sustain,
    #[default]
    Release,
}

/// One operator's registers and state.
#[derive(Clone, Debug)]
pub struct Operator {
    pub detune: u8,
    pub multiple: u8,
    pub total_level: u8,
    pub key_scale: u8,
    pub attack: u8,
    pub decay: u8,
    pub sustain_rate: u8,
    pub sustain_level: u8,
    pub release: u8,
    pub am: bool,
    pub ssg: u8,
    phase: u32,
    /// Attenuation, 0 (loudest) to 1023 (silent).
    env: i32,
    stage: Stage,
    key: bool,
    ssg_inverted: bool,
    ssg_held: bool,
    key_code: u32,
}

impl Default for Operator {
    fn default() -> Operator {
        Operator {
            detune: 0,
            multiple: 0,
            total_level: 0,
            key_scale: 0,
            attack: 0,
            decay: 0,
            sustain_rate: 0,
            sustain_level: 0,
            release: 0,
            am: false,
            ssg: 0,
            phase: 0,
            env: 1023,
            stage: Stage::Release,
            key: false,
            ssg_inverted: false,
            ssg_held: false,
            key_code: 0,
        }
    }
}

impl Operator {
    /// An envelope rate register (5 bits, or release's 4 doubled plus one) as
    /// an effective rate, with key scaling.
    fn rate(&self, raw: u32) -> u32 {
        if raw == 0 {
            0
        } else {
            (raw * 2 + (self.key_code >> (3 - self.key_scale as u32))).min(63)
        }
    }

    fn sustain_attenuation(&self) -> i32 {
        if self.sustain_level == 15 { 0x3E0 } else { (self.sustain_level as i32) << 5 }
    }

    fn key_on(&mut self) {
        if self.key {
            return;
        }
        self.key = true;
        self.phase = 0;
        self.ssg_inverted = false;
        self.ssg_held = false;
        self.stage = Stage::Attack;
        if self.rate(self.attack as u32) >= 62 {
            self.env = 0;
            self.stage = if self.sustain_attenuation() == 0 { Stage::Sustain } else { Stage::Decay };
        }
    }

    fn key_off(&mut self) {
        if !self.key {
            return;
        }
        self.key = false;
        if self.ssg & 8 != 0 && self.output_inverted() {
            self.env = (0x200 - self.env).clamp(0, 1023);
        }
        self.ssg_inverted = false;
        self.ssg_held = false;
        self.stage = Stage::Release;
    }

    fn output_inverted(&self) -> bool {
        self.ssg & 8 != 0 && (self.ssg_inverted ^ (self.ssg & 4 != 0))
    }

    /// One envelope tick (every third sample), at envelope counter `count`.
    fn envelope_tick(&mut self, count: u32) {
        let ssg = self.ssg & 8 != 0;
        // SSG-EG: past half way, repeat, alternate or hold.
        if ssg && self.stage != Stage::Release && self.stage != Stage::Attack && self.env >= 0x200 {
            if self.ssg & 1 != 0 {
                if !self.ssg_held {
                    self.ssg_held = true;
                    if self.ssg & 2 != 0 {
                        self.ssg_inverted = !self.ssg_inverted;
                    }
                }
                self.env = 0x200;
                return;
            }
            if self.ssg & 2 != 0 {
                self.ssg_inverted = !self.ssg_inverted;
            } else {
                self.phase = 0;
            }
            self.stage = Stage::Attack;
            if self.rate(self.attack as u32) >= 62 {
                self.env = 0;
                self.stage = Stage::Decay;
            }
            return;
        }
        let raw = match self.stage {
            Stage::Attack => self.attack as u32,
            Stage::Decay => self.decay as u32,
            Stage::Sustain => self.sustain_rate as u32,
            Stage::Release => self.release as u32 * 2 + 1,
        };
        let rate = self.rate(raw);
        let (shift, pattern) = rate_step(rate);
        if count & ((1 << shift) - 1) != 0 {
            return;
        }
        let inc = EG_INC[pattern][((count >> shift) & 7) as usize] as i32;
        match self.stage {
            Stage::Attack => {
                if rate >= 62 {
                    self.env = 0;
                } else if inc > 0 {
                    self.env += (!self.env * inc) >> 4;
                }
                if self.env <= 0 {
                    self.env = 0;
                    self.stage = Stage::Decay;
                }
            }
            _ => {
                let step = if ssg && self.env < 0x200 { inc * 4 } else { inc };
                let limit = if ssg && self.stage != Stage::Release { 0x200 } else { 1023 };
                self.env = (self.env + step).min(limit.max(self.env));
                if self.stage == Stage::Release {
                    self.env = self.env.min(1023);
                }
            }
        }
        if self.stage == Stage::Decay && self.env >= self.sustain_attenuation() {
            self.stage = Stage::Sustain;
        }
    }

    /// Total attenuation, 0–1023: envelope (inverted for SSG-EG), total
    /// level, and the LFO's tremolo.
    fn attenuation(&self, tremolo: u32) -> u32 {
        let env = if self.ssg_held && self.ssg & 8 != 0 {
            if self.output_inverted() { 0 } else { 1023 }
        } else if self.output_inverted() {
            (0x200 - self.env).clamp(0, 1023)
        } else {
            self.env
        };
        let am = if self.am { tremolo } else { 0 };
        (env as u32 + ((self.total_level as u32) << 3) + am).min(1023)
    }
}

/// The operator's output, −8191 to 8191, for a 10-bit phase and an
/// attenuation.
fn operator_output(phase: u32, attenuation: u32) -> i32 {
    let t = tables();
    let p = phase & 0x3FF;
    let q = if p & 0x100 != 0 { !p & 0xFF } else { p & 0xFF };
    let level = t.logsin[q as usize] as u32 + (attenuation << 2);
    let shift = level >> 8;
    let out = if shift > 13 { 0 } else { (((t.exp[(level & 0xFF) as usize] as u32) | 0x400) << 2) >> shift };
    if p & 0x200 != 0 { -(out as i32) } else { out as i32 }
}

/// One channel's registers and state. Operators are kept in algorithm
/// order: operator 1, 2, 3, 4.
#[derive(Clone, Debug)]
pub struct Channel {
    pub fnum: u16,
    pub block: u8,
    pub algorithm: u8,
    pub feedback: u8,
    pub left: bool,
    pub right: bool,
    pub ams: u8,
    pub pms: u8,
    pub ops: [Operator; 4],
    feedback_out: [i32; 2],
    /// The last output, 9-bit.
    pub out: i32,
}

impl Default for Channel {
    fn default() -> Channel {
        Channel {
            fnum: 0,
            block: 0,
            algorithm: 0,
            feedback: 0,
            left: false,
            right: false,
            ams: 0,
            pms: 0,
            ops: Default::default(),
            feedback_out: [0; 2],
            out: 0,
        }
    }
}

/// Register offset (0, 4, 8, 12) to operator: the chip's slots run 1, 3, 2, 4.
const SLOT_TO_OP: [usize; 4] = [0, 2, 1, 3];

/// The chip.
pub struct Ym2612 {
    pub channels: [Channel; 6],
    /// Channel 3's special mode: a frequency for operators 1–3.
    pub ch3_mode: u8,
    ch3_fnum: [u16; 3],
    ch3_block: [u8; 3],
    latch: [u8; 2],
    ch3_latch: u8,
    lfo_on: bool,
    lfo_rate: u8,
    lfo_count: u32,
    lfo_step: u32,
    eg_count: u32,
    eg_divider: u32,
    pub dac_on: bool,
    pub dac: u8,
    /// Output: rate, resampling position and the last native sample.
    rate: u32,
    position: f64,
    last: (f32, f32),
    samples: Vec<f32>,
    /// One-pole output filters: an optional low-pass, and a DC blocker.
    lowpass: Option<f32>,
    lp_state: (f32, f32),
    dc_state: (f32, f32, f32, f32),
    pub ladder: bool,
}

impl Ym2612 {
    /// A chip at power on, rendering `rate` samples a second.
    pub fn new(rate: u32) -> Ym2612 {
        Ym2612 {
            channels: Default::default(),
            ch3_mode: 0,
            ch3_fnum: [0; 3],
            ch3_block: [0; 3],
            latch: [0; 2],
            ch3_latch: 0,
            lfo_on: false,
            lfo_rate: 0,
            lfo_count: 0,
            lfo_step: 0,
            eg_count: 0,
            eg_divider: 0,
            dac_on: false,
            dac: 0x80,
            rate,
            position: 0.0,
            last: (0.0, 0.0),
            samples: Vec::new(),
            lowpass: None,
            lp_state: (0.0, 0.0),
            dc_state: (0.0, 0.0, 0.0, 0.0),
            ladder: true,
        }
    }

    /// A first-order low-pass on the output at `hz`, or none.
    pub fn set_lowpass(&mut self, hz: Option<f32>) {
        self.lowpass = hz;
    }

    /// Write `data` to register `addr` on `port` 0 (channels 1–3 and the
    /// globals) or 1 (channels 4–6).
    pub fn write(&mut self, port: u8, addr: u8, data: u8) {
        let port = (port & 1) as usize;
        if port == 0 && addr < 0x30 {
            self.write_global(addr, data);
            return;
        }
        let c = (addr & 3) as usize;
        if c == 3 {
            return;
        }
        let ch = port * 3 + c;
        if (0x30..0xA0).contains(&addr) {
            let op = &mut self.channels[ch].ops[SLOT_TO_OP[((addr >> 2) & 3) as usize]];
            match addr & 0xF0 {
                0x30 => {
                    op.detune = (data >> 4) & 7;
                    op.multiple = data & 15;
                }
                0x40 => op.total_level = data & 0x7F,
                0x50 => {
                    op.key_scale = data >> 6;
                    op.attack = data & 31;
                }
                0x60 => {
                    op.am = data & 0x80 != 0;
                    op.decay = data & 31;
                }
                0x70 => op.sustain_rate = data & 31,
                0x80 => {
                    op.sustain_level = data >> 4;
                    op.release = data & 15;
                }
                0x90 => op.ssg = data & 15,
                _ => {}
            }
            return;
        }
        match addr & 0xFC {
            0xA4 => self.latch[port] = data & 0x3F,
            0xA0 => {
                let l = self.latch[port];
                let chan = &mut self.channels[ch];
                chan.fnum = ((l as u16 & 7) << 8) | data as u16;
                chan.block = (l >> 3) & 7;
            }
            0xAC if port == 0 => self.ch3_latch = data & 0x3F,
            0xA8 if port == 0 => {
                // $A8 is operator 3, $A9 operator 1, $AA operator 2.
                let op = [2, 0, 1][c];
                self.ch3_fnum[op] = ((self.ch3_latch as u16 & 7) << 8) | data as u16;
                self.ch3_block[op] = (self.ch3_latch >> 3) & 7;
            }
            0xB0 => {
                let chan = &mut self.channels[ch];
                chan.feedback = (data >> 3) & 7;
                chan.algorithm = data & 7;
            }
            0xB4 => {
                let chan = &mut self.channels[ch];
                chan.left = data & 0x80 != 0;
                chan.right = data & 0x40 != 0;
                chan.ams = (data >> 4) & 3;
                chan.pms = data & 7;
            }
            _ => {}
        }
    }

    fn write_global(&mut self, addr: u8, data: u8) {
        match addr {
            0x22 => {
                self.lfo_on = data & 8 != 0;
                self.lfo_rate = data & 7;
                if !self.lfo_on {
                    self.lfo_step = 0;
                    self.lfo_count = 0;
                }
            }
            0x27 => self.ch3_mode = data >> 6,
            0x28 => {
                let ch = match data & 7 {
                    c @ 0..=2 => c as usize,
                    c @ 4..=6 => c as usize - 1,
                    _ => return,
                };
                // Bits 4–7 key operators 1, 2, 3, 4.
                for op in 0..4 {
                    if data & (0x10 << op) != 0 {
                        self.channels[ch].ops[op].key_on();
                    } else {
                        self.channels[ch].ops[op].key_off();
                    }
                }
            }
            0x2A => self.dac = data,
            0x2B => self.dac_on = data & 0x80 != 0,
            _ => {}
        }
    }

    /// Write a register by its 9-bit number: `$000`–`$0FF` port 0,
    /// `$100`–`$1FF` port 1.
    pub fn write_reg(&mut self, reg: u16, data: u8) {
        self.write((reg >> 8) as u8, reg as u8, data);
    }

    /// The LFO's tremolo (0–126) and vibrato (−1 to 1) now.
    fn lfo(&self) -> (u32, f32) {
        if !self.lfo_on {
            return (0, 0.0);
        }
        let s = self.lfo_step;
        let am = if s < 64 { s * 2 } else { 252 - s * 2 };
        let pm = match s {
            0..=31 => s as f32 / 32.0,
            32..=95 => (64.0 - s as f32) / 32.0,
            _ => (s as f32 - 128.0) / 32.0,
        };
        (am.min(126), pm)
    }

    /// One sample at the chip's own rate.
    pub fn step(&mut self) {
        if self.lfo_on {
            self.lfo_count += 1;
            if self.lfo_count >= LFO_PERIOD[self.lfo_rate as usize] {
                self.lfo_count = 0;
                self.lfo_step = (self.lfo_step + 1) & 127;
            }
        }
        self.eg_divider += 1;
        let eg_tick = self.eg_divider == 3;
        if eg_tick {
            self.eg_divider = 0;
            self.eg_count = (self.eg_count + 1) & 0xFFF;
            if self.eg_count == 0 {
                self.eg_count = 1;
            }
        }
        let (am, pm) = self.lfo();
        let (mut left, mut right) = (0i32, 0i32);
        for ch in 0..6 {
            let special = ch == 2 && self.ch3_mode != 0;
            let chan = &mut self.channels[ch];
            // Pitch: each operator's F-number and block (channel 3's own in
            // special mode), with vibrato, detune and multiple.
            for (i, op) in chan.ops.iter_mut().enumerate() {
                let (mut fnum, block) = if special && i < 3 { (self.ch3_fnum[i], self.ch3_block[i]) } else { (chan.fnum, chan.block) };
                if chan.pms > 0 && pm != 0.0 {
                    let cents = PM_CENTS[chan.pms as usize] * pm;
                    let f = fnum as f32 * 2f32.powf(cents / 1200.0);
                    fnum = (f.round() as i32).clamp(0, 2047) as u16;
                }
                op.key_code = ((block as u32) << 2) | KEY_FRACTION[(fnum >> 7) as usize] as u32;
                let mut base = ((fnum as u32) << block) >> 1;
                let dt = DETUNE[(op.detune & 3) as usize][op.key_code as usize] as u32;
                base = if op.detune & 4 != 0 { base.wrapping_sub(dt) } else { base + dt } & 0x1FFFF;
                let inc = if op.multiple == 0 { base >> 1 } else { base * op.multiple as u32 };
                op.phase = (op.phase + inc) & 0xFFFFF;
                if eg_tick {
                    op.envelope_tick(self.eg_count);
                }
            }
            let tremolo = am >> AM_SHIFT[chan.ams as usize];
            let ops = &chan.ops;
            let run = |i: usize, modulation: i32| -> i32 {
                let op = &ops[i];
                operator_output(((op.phase >> 10) as i32 + modulation) as u32, op.attenuation(tremolo))
            };
            let fb = if chan.feedback > 0 { (chan.feedback_out[0] + chan.feedback_out[1]) >> (10 - chan.feedback) } else { 0 };
            let o1 = run(0, fb);
            let out = match chan.algorithm {
                0 => {
                    let o2 = run(1, o1 >> 1);
                    let o3 = run(2, o2 >> 1);
                    run(3, o3 >> 1)
                }
                1 => {
                    let o2 = run(1, 0);
                    let o3 = run(2, (o1 + o2) >> 1);
                    run(3, o3 >> 1)
                }
                2 => {
                    let o2 = run(1, 0);
                    let o3 = run(2, o2 >> 1);
                    run(3, (o1 + o3) >> 1)
                }
                3 => {
                    let o2 = run(1, o1 >> 1);
                    let o3 = run(2, 0);
                    run(3, (o2 + o3) >> 1)
                }
                4 => {
                    let o2 = run(1, o1 >> 1);
                    let o3 = run(2, 0);
                    o2 + run(3, o3 >> 1)
                }
                5 => run(1, o1 >> 1) + run(2, o1 >> 1) + run(3, o1 >> 1),
                6 => run(1, o1 >> 1) + run(2, 0) + run(3, 0),
                _ => o1 + run(1, 0) + run(2, 0) + run(3, 0),
            };
            chan.feedback_out = [o1, chan.feedback_out[0]];
            let mut v = out.clamp(-8192, 8191) >> 5;
            if ch == 5 && self.dac_on {
                v = (self.dac as i32 - 128) << 1;
            }
            chan.out = v;
            let analog = if self.ladder { if v >= 0 { v + LADDER } else { v - LADDER } } else { v };
            if chan.left {
                left += analog;
            }
            if chan.right {
                right += analog;
            }
        }
        // Six 9-bit channels: one at full scale is about a third of the range.
        let (l, r) = (left as f32 / 768.0, right as f32 / 768.0);
        self.output(l, r);
    }

    fn output(&mut self, l: f32, r: f32) {
        let (mut l, mut r) = (l, r);
        if let Some(hz) = self.lowpass {
            let a = 1.0 - (-2.0 * std::f32::consts::PI * hz / CHIP_HZ as f32).exp();
            self.lp_state.0 += (l - self.lp_state.0) * a;
            self.lp_state.1 += (r - self.lp_state.1) * a;
            l = self.lp_state.0;
            r = self.lp_state.1;
        }
        // DC blocker (about 10 Hz): the ladder offset is not heard.
        let k = 0.9988;
        let (pl, pr, ol, or) = self.dc_state;
        let yl = l - pl + k * ol;
        let yr = r - pr + k * or;
        self.dc_state = (l, r, yl, yr);
        let step = CHIP_HZ / self.rate as f64;
        while self.position <= 1.0 {
            let t = self.position as f32;
            self.samples.push(self.last.0 + (yl - self.last.0) * t);
            self.samples.push(self.last.1 + (yr - self.last.1) * t);
            self.position += step;
        }
        self.position -= 1.0;
        self.last = (yl, yr);
    }

    /// Run `n` samples at the chip's rate.
    pub fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.step();
        }
    }

    /// The samples made so far at the output rate, interleaved left, right.
    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }

    /// How loud a channel's carriers are now, 0–15, from their envelopes and
    /// total levels (for meters).
    pub fn level(&self, ch: usize) -> u8 {
        let chan = &self.channels[ch];
        if ch == 5 && self.dac_on {
            return ((self.dac as i32 - 128).unsigned_abs() * 15 / 128).min(15) as u8;
        }
        let carriers: &[usize] = match chan.algorithm {
            0..=3 => &[3],
            4 => &[1, 3],
            5 | 6 => &[1, 2, 3],
            _ => &[0, 1, 2, 3],
        };
        let att = carriers.iter().map(|&i| chan.ops[i].attenuation(0)).min().unwrap_or(1023);
        // 1023 steps of 0.09375 dB; 15 meter steps over the top 48 dB.
        15u32.saturating_sub(att / 34) as u8
    }

    /// An operator's envelope stage (for tests and displays).
    pub fn stage(&self, ch: usize, op: usize) -> Stage {
        self.channels[ch].ops[op].stage
    }
}

/// A note's frequency in Hz (MIDI number; A4 = 69 = 440 Hz).
pub fn note_hz(midi: i32) -> f64 {
    440.0 * 2f64.powf((midi - 69) as f64 / 12.0)
}

/// The block and F-number for `hz`: the lowest block that holds it, for the
/// finest steps.
pub fn fnum_for(hz: f64) -> (u8, u16) {
    for block in 0..8u8 {
        let f = hz * (1u64 << 21) as f64 / CHIP_HZ / (1u32 << block) as f64;
        if f < 2047.5 {
            return (block, f.round() as u16);
        }
    }
    (7, 2047)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44100;

    /// A plain sine on channel `ch` (algorithm 7, only operator 4 audible).
    fn sine(chip: &mut Ym2612, ch: usize, hz: f64) {
        let (port, c) = ((ch / 3) as u8, (ch % 3) as u8);
        chip.write(port, 0xB0 + c, 0x07);
        chip.write(port, 0xB4 + c, 0xC0);
        for slot in 0..4u8 {
            let r = slot * 4 + c;
            chip.write(port, 0x30 + r, 0x01);
            chip.write(port, 0x40 + r, if slot == 3 { 0 } else { 127 });
            chip.write(port, 0x50 + r, 0x1F);
            chip.write(port, 0x60 + r, 0);
            chip.write(port, 0x70 + r, 0);
            chip.write(port, 0x80 + r, 0x0F);
        }
        let (block, fnum) = fnum_for(hz);
        chip.write(port, 0xA4 + c, (block << 3) | (fnum >> 8) as u8);
        chip.write(port, 0xA0 + c, fnum as u8);
        let key = if ch < 3 { ch as u8 } else { ch as u8 + 1 };
        chip.write(0, 0x28, 0xF0 | key);
    }

    fn left(s: &[f32]) -> Vec<f32> {
        s.iter().step_by(2).copied().collect()
    }

    fn crossings(s: &[f32]) -> usize {
        s.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count()
    }

    fn peak(s: &[f32]) -> f32 {
        s.iter().fold(0.0f32, |a, v| a.max(v.abs()))
    }

    #[test]
    fn tables_match_the_chip() {
        let t = tables();
        assert_eq!(t.logsin[255], 0);
        assert_eq!(t.logsin[0], 2137);
        assert_eq!(t.exp[0], 1018);
        assert_eq!(t.exp[255], 0);
        assert_eq!(operator_output(0x100, 0), 8168);
        assert_eq!(operator_output(0x300, 0), -8168);
        assert_eq!(operator_output(0x100, 1023), 0);
    }

    #[test]
    fn a440_is_440() {
        assert_eq!(fnum_for(440.0), (4, 1083));
        let mut chip = Ym2612::new(RATE);
        sine(&mut chip, 0, 440.0);
        chip.run(CHIP_HZ as u32);
        let s = left(&chip.take_samples());
        assert!((s.len() as i64 - RATE as i64).abs() <= 2, "{}", s.len());
        let n = crossings(&s[100..]);
        assert!((438..=442).contains(&n), "{n}");
    }

    #[test]
    fn multiple_doubles_and_half() {
        for (mul, expect) in [(2u8, 880), (0, 220)] {
            let mut chip = Ym2612::new(RATE);
            sine(&mut chip, 1, 440.0);
            chip.write(0, 0x3D, mul);
            chip.run(CHIP_HZ as u32);
            let n = crossings(&left(&chip.take_samples())[100..]);
            assert!((n as i32 - expect).abs() <= 3, "mul {mul}: {n}");
        }
    }

    #[test]
    fn eight_total_level_steps_halve_the_wave() {
        let loud = {
            let mut chip = Ym2612::new(RATE);
            sine(&mut chip, 0, 220.0);
            chip.run(10000);
            peak(&left(&chip.take_samples())[4000..])
        };
        let quiet = {
            let mut chip = Ym2612::new(RATE);
            sine(&mut chip, 0, 220.0);
            chip.write(0, 0x4C, 8);
            chip.run(10000);
            peak(&left(&chip.take_samples())[4000..])
        };
        let ratio = quiet / loud;
        assert!((ratio - 0.5).abs() < 0.03, "{ratio}");
    }

    #[test]
    fn envelopes_attack_decay_and_release() {
        let mut chip = Ym2612::new(RATE);
        sine(&mut chip, 0, 220.0);
        // A slow attack, then a decay to sustain level 4 (12 dB down).
        chip.write(0, 0x28, 0);
        chip.run(6000);
        chip.take_samples();
        chip.write(0, 0x5C, 0x10);
        chip.write(0, 0x6C, 0x10);
        chip.write(0, 0x8C, 0x4F);
        chip.write(0, 0x28, 0xF0);
        assert_eq!(chip.stage(0, 3), Stage::Attack);
        chip.run(400);
        let early = peak(&left(&chip.take_samples()));
        chip.run(20000);
        assert_ne!(chip.stage(0, 3), Stage::Attack);
        chip.run(40000);
        assert_eq!(chip.stage(0, 3), Stage::Sustain);
        chip.take_samples();
        chip.run(2000);
        let held = peak(&left(&chip.take_samples()));
        assert!(early < held, "{early} {held}");
        // Sustain level 4 is 12 dB, a quarter of full.
        let full = 8180.0 / 32.0 / 768.0 * 4.0 / 4.0;
        assert!((held / full - 0.25).abs() < 0.06, "{held} {full}");
        chip.write(0, 0x28, 0);
        chip.run(6000);
        chip.take_samples();
        chip.run(1000);
        assert!(peak(&left(&chip.take_samples())) < 0.002);
    }

    #[test]
    fn a_modulator_adds_harmonics() {
        let mut chip = Ym2612::new(RATE);
        sine(&mut chip, 0, 220.0);
        // Algorithm 0: operator 3 (slot $38) modulates 4 at three times the pitch.
        chip.write(0, 0xB0, 0x00);
        chip.write(0, 0x38, 0x03);
        chip.write(0, 0x48, 20);
        chip.run(CHIP_HZ as u32 / 2);
        let s = left(&chip.take_samples());
        assert!((108..=112).contains(&crossings(&s[100..])), "{}", crossings(&s[100..]));
        // Not a sine: its peak sample set holds more than two turning points a cycle.
        let turns = s.windows(3).filter(|w| (w[1] - w[0]) * (w[2] - w[1]) < 0.0).count();
        assert!(turns > 110 * 4, "{turns}");
    }

    #[test]
    fn panning_and_the_dac() {
        let mut chip = Ym2612::new(RATE);
        sine(&mut chip, 0, 220.0);
        chip.write(0, 0xB4, 0x80);
        chip.run(4000);
        let s = chip.take_samples();
        let l = peak(&s.iter().step_by(2).copied().collect::<Vec<_>>());
        let r = peak(&s.iter().skip(1).step_by(2).copied().collect::<Vec<_>>());
        assert!(l > 0.2 && r < 0.01, "{l} {r}");
        let mut chip = Ym2612::new(RATE);
        chip.write(1, 0xB6, 0xC0);
        chip.write(0, 0x2B, 0x80);
        assert_eq!(chip.level(5), 0);
        for i in 0..2000u32 {
            chip.write(0, 0x2A, if (i / 40) % 2 == 0 { 0xF0 } else { 0x10 });
            chip.step();
        }
        assert!(peak(&left(&chip.take_samples())) > 0.2);
    }

    #[test]
    fn channel_3_special_mode_sets_each_operator() {
        let mut chip = Ym2612::new(RATE);
        sine(&mut chip, 2, 440.0);
        // Special mode; operator 4 keeps the channel's 440 Hz; make 4 silent
        // and 1 audible at its own 660 Hz.
        chip.write(0, 0x27, 0x40);
        chip.write(0, 0x4E, 127);
        chip.write(0, 0x42, 0);
        let (block, fnum) = fnum_for(660.0);
        chip.write(0, 0xAD, (block << 3) | (fnum >> 8) as u8);
        chip.write(0, 0xA9, fnum as u8);
        chip.run(CHIP_HZ as u32);
        let n = crossings(&left(&chip.take_samples())[100..]);
        assert!((n as i32 - 660).abs() <= 3, "{n}");
    }

    #[test]
    fn the_lfo_trembles() {
        let mut chip = Ym2612::new(RATE);
        sine(&mut chip, 0, 440.0);
        chip.write(0, 0x22, 0x08 | 7);
        chip.write(0, 0xB4, 0xC0 | 0x30);
        chip.write(0, 0x6C, 0x80);
        chip.run(CHIP_HZ as u32 / 4);
        let s = left(&chip.take_samples());
        let chunks: Vec<f32> = s[2000..].chunks(60).map(peak).collect();
        let (lo, hi) = chunks.iter().fold((1.0f32, 0.0f32), |a, &v| (a.0.min(v), a.1.max(v)));
        assert!(lo < hi * 0.4, "{lo} {hi}");
    }

    #[test]
    fn ssg_eg_repeats() {
        let mut chip = Ym2612::new(RATE);
        sine(&mut chip, 0, 440.0);
        chip.write(0, 0x28, 0);
        chip.run(6000);
        chip.take_samples();
        chip.write(0, 0x6C, 0x12);
        chip.write(0, 0x8C, 0xFF);
        chip.write(0, 0x9C, 0x08);
        chip.write(0, 0x28, 0xF0);
        chip.run(CHIP_HZ as u32 / 2);
        let s = left(&chip.take_samples());
        let chunks: Vec<f32> = s.chunks(200).map(peak).collect();
        // It decays and starts over: the loudness rises again many times.
        let rises = chunks.windows(2).filter(|w| w[1] > w[0] * 2.0 && w[1] > 0.05).count();
        assert!(rises >= 3, "{rises}");
    }
}
