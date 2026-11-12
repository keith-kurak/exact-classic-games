//! The NES's audio processing unit (the 2A03's APU), emulated cycle by cycle
//! and driven the way a game drives it: by writing its registers, $4000–$4017.
//!
//! - Two pulse channels: four duty cycles, a volume envelope, a sweep unit
//!   (pulse 1 sweeps down by ones' complement, pulse 2 by two's), a length
//!   counter. Periods under 8, or a sweep target past $7FF, mute.
//! - A triangle: the 32-step wave, its linear counter and length counter.
//! - Noise: the 15-bit shift register, long and short (metallic) modes, the
//!   16 NTSC periods, an envelope and length counter.
//! - The DMC's 7-bit output level ($4011), for direct sample writes; its
//!   sample fetching (DMA from cartridge memory) is not emulated.
//! - The frame counter: 4- and 5-step sequences clocking envelopes and the
//!   linear counter (quarter frames) and lengths and sweeps (half frames).
//! - The mixer: the console's non-linear DAC curves, then its high-pass
//!   (90 Hz, 440 Hz) and low-pass (14 kHz) output filters.
//!
//! [`Apu::write`] a register, [`Apu::run`] some CPU cycles, and read samples.
//! [`driver`] is a small 60 Hz sound driver (instruments, notes, songs) on
//! top; [`wav`] writes the result as a WAV file.

pub mod driver;
pub mod wav;

/// The NTSC CPU clock, in Hz.
pub const CPU_HZ: f64 = 1_789_773.0;
/// NTSC frames a second.
pub const FRAME_HZ: f64 = 60.0988;

const LENGTHS: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30,
];
const DUTIES: [[u8; 8]; 4] = [[0, 1, 0, 0, 0, 0, 0, 0], [0, 1, 1, 0, 0, 0, 0, 0], [0, 1, 1, 1, 1, 0, 0, 0], [1, 0, 0, 1, 1, 1, 1, 1]];
const TRIANGLE: [u8; 32] = [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const NOISE_PERIODS: [u16; 16] = [4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068];

#[derive(Clone, Debug, Default)]
struct Envelope {
    start: bool,
    looped: bool,
    constant: bool,
    volume: u8,
    divider: u8,
    decay: u8,
}

impl Envelope {
    fn write(&mut self, v: u8) {
        self.looped = v & 0x20 != 0;
        self.constant = v & 0x10 != 0;
        self.volume = v & 0x0F;
    }
    fn quarter(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.volume;
        } else if self.divider == 0 {
            self.divider = self.volume;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.looped {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }
    fn output(&self) -> u8 {
        if self.constant { self.volume } else { self.decay }
    }
}

#[derive(Clone, Debug, Default)]
struct Pulse {
    second: bool,
    enabled: bool,
    duty: u8,
    step: u8,
    period: u16,
    timer: u16,
    length: u8,
    halt: bool,
    env: Envelope,
    sweep_on: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_divider: u8,
    sweep_reload: bool,
}

impl Pulse {
    fn target(&self) -> i32 {
        let change = (self.period >> self.sweep_shift) as i32;
        if self.sweep_negate {
            self.period as i32 - change - if self.second { 0 } else { 1 }
        } else {
            self.period as i32 + change
        }
    }
    fn muted(&self) -> bool {
        self.period < 8 || self.target() > 0x7FF
    }
    fn write(&mut self, reg: u16, v: u8) {
        match reg & 3 {
            0 => {
                self.duty = v >> 6;
                self.halt = v & 0x20 != 0;
                self.env.write(v);
            }
            1 => {
                self.sweep_on = v & 0x80 != 0;
                self.sweep_period = (v >> 4) & 7;
                self.sweep_negate = v & 0x08 != 0;
                self.sweep_shift = v & 7;
                self.sweep_reload = true;
            }
            2 => self.period = (self.period & 0x700) | v as u16,
            _ => {
                self.period = (self.period & 0xFF) | ((v as u16 & 7) << 8);
                if self.enabled {
                    self.length = LENGTHS[(v >> 3) as usize];
                }
                // Writing the high byte restarts the waveform and the envelope:
                // the faint click of a retriggered NES note.
                self.step = 0;
                self.env.start = true;
            }
        }
    }
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            self.step = (self.step + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }
    fn half(&mut self) {
        if !self.halt && self.length > 0 {
            self.length -= 1;
        }
        let target = self.target();
        if self.sweep_divider == 0 && self.sweep_on && self.sweep_shift > 0 && !self.muted() && target >= 0 {
            self.period = target as u16;
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }
    fn output(&self) -> u8 {
        if self.length == 0 || self.muted() || DUTIES[self.duty as usize][self.step as usize] == 0 {
            0
        } else {
            self.env.output()
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Triangle {
    enabled: bool,
    control: bool,
    reload_value: u8,
    linear: u8,
    reload: bool,
    period: u16,
    timer: u16,
    step: u8,
    length: u8,
}

impl Triangle {
    fn write(&mut self, reg: u16, v: u8) {
        match reg & 3 {
            0 => {
                self.control = v & 0x80 != 0;
                self.reload_value = v & 0x7F;
            }
            2 => self.period = (self.period & 0x700) | v as u16,
            3 => {
                self.period = (self.period & 0xFF) | ((v as u16 & 7) << 8);
                if self.enabled {
                    self.length = LENGTHS[(v >> 3) as usize];
                }
                self.reload = true;
            }
            _ => {}
        }
    }
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            // Very short periods would whine above hearing; real hardware
            // plays them, but games avoid them, and so does this.
            if self.linear > 0 && self.length > 0 && self.period >= 2 {
                self.step = (self.step + 1) & 31;
            }
        } else {
            self.timer -= 1;
        }
    }
    fn quarter(&mut self) {
        if self.reload {
            self.linear = self.reload_value;
        } else if self.linear > 0 {
            self.linear -= 1;
        }
        if !self.control {
            self.reload = false;
        }
    }
    fn half(&mut self) {
        if !self.control && self.length > 0 {
            self.length -= 1;
        }
    }
    fn output(&self) -> u8 {
        TRIANGLE[self.step as usize]
    }
}

#[derive(Clone, Debug)]
struct Noise {
    enabled: bool,
    short: bool,
    period: u16,
    timer: u16,
    shift: u16,
    length: u8,
    halt: bool,
    env: Envelope,
}

impl Default for Noise {
    fn default() -> Self {
        Noise { enabled: false, short: false, period: NOISE_PERIODS[0], timer: 0, shift: 1, length: 0, halt: false, env: Envelope::default() }
    }
}

impl Noise {
    fn write(&mut self, reg: u16, v: u8) {
        match reg & 3 {
            0 => {
                self.halt = v & 0x20 != 0;
                self.env.write(v);
            }
            2 => {
                self.short = v & 0x80 != 0;
                self.period = NOISE_PERIODS[(v & 0x0F) as usize];
            }
            3 => {
                if self.enabled {
                    self.length = LENGTHS[(v >> 3) as usize];
                }
                self.env.start = true;
            }
            _ => {}
        }
    }
    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.period;
            let tap = if self.short { 6 } else { 1 };
            let feedback = (self.shift & 1) ^ ((self.shift >> tap) & 1);
            self.shift = (self.shift >> 1) | (feedback << 14);
        } else {
            self.timer -= 1;
        }
    }
    fn half(&mut self) {
        if !self.halt && self.length > 0 {
            self.length -= 1;
        }
    }
    fn output(&self) -> u8 {
        if self.length == 0 || self.shift & 1 != 0 { 0 } else { self.env.output() }
    }
}

/// A first-order filter, as the console's output stage is modelled.
#[derive(Clone, Debug)]
struct Filter {
    high_pass: bool,
    k: f64,
    prev_in: f64,
    prev_out: f64,
}

impl Filter {
    fn new(high_pass: bool, cutoff: f64, rate: f64) -> Filter {
        let rc = 1.0 / (2.0 * std::f64::consts::PI * cutoff);
        let dt = 1.0 / rate;
        let k = if high_pass { rc / (rc + dt) } else { dt / (rc + dt) };
        Filter { high_pass, k, prev_in: 0.0, prev_out: 0.0 }
    }
    fn step(&mut self, x: f64) -> f64 {
        let y = if self.high_pass { self.k * (self.prev_out + x - self.prev_in) } else { self.prev_out + self.k * (x - self.prev_out) };
        self.prev_in = x;
        self.prev_out = y;
        y
    }
}

/// What each channel is doing now, for meters: its 0–15 level, and its
/// timer period (0 when silent).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Channels {
    pub pulse1: (u8, u16),
    pub pulse2: (u8, u16),
    pub triangle: (u8, u16),
    pub noise: (u8, u16),
}

/// The APU, and the samples it has made since they were last taken.
pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc_level: u8,
    five_step: bool,
    frame_cycle: u32,
    odd: bool,
    rate: f64,
    cycles_per_sample: f64,
    owed: f64,
    acc: f64,
    acc_n: u32,
    filters: [Filter; 3],
    samples: Vec<f32>,
}

impl Apu {
    /// An APU that makes `rate` samples a second (44100 is usual).
    pub fn new(rate: u32) -> Apu {
        let rate = rate as f64;
        Apu {
            pulse1: Pulse::default(),
            pulse2: Pulse { second: true, ..Pulse::default() },
            triangle: Triangle::default(),
            noise: Noise::default(),
            dmc_level: 0,
            five_step: false,
            frame_cycle: 0,
            odd: false,
            rate,
            cycles_per_sample: CPU_HZ / rate,
            owed: CPU_HZ / rate,
            acc: 0.0,
            acc_n: 0,
            filters: [Filter::new(true, 90.0, rate), Filter::new(true, 440.0, rate), Filter::new(false, 14_000.0, rate)],
            samples: Vec::new(),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.rate as u32
    }

    /// Write a register, $4000–$4017, as the CPU would.
    pub fn write(&mut self, addr: u16, v: u8) {
        match addr {
            0x4000..=0x4003 => self.pulse1.write(addr, v),
            0x4004..=0x4007 => self.pulse2.write(addr, v),
            0x4008..=0x400B => self.triangle.write(addr, v),
            0x400C..=0x400F => self.noise.write(addr, v),
            0x4011 => self.dmc_level = v & 0x7F,
            0x4015 => {
                self.pulse1.enabled = v & 1 != 0;
                self.pulse2.enabled = v & 2 != 0;
                self.triangle.enabled = v & 4 != 0;
                self.noise.enabled = v & 8 != 0;
                if !self.pulse1.enabled {
                    self.pulse1.length = 0;
                }
                if !self.pulse2.enabled {
                    self.pulse2.length = 0;
                }
                if !self.triangle.enabled {
                    self.triangle.length = 0;
                }
                if !self.noise.enabled {
                    self.noise.length = 0;
                }
            }
            0x4017 => {
                self.five_step = v & 0x80 != 0;
                self.frame_cycle = 0;
                if self.five_step {
                    self.quarter();
                    self.half();
                }
            }
            _ => {}
        }
    }

    fn quarter(&mut self) {
        self.pulse1.env.quarter();
        self.pulse2.env.quarter();
        self.noise.env.quarter();
        self.triangle.quarter();
    }

    fn half(&mut self) {
        self.pulse1.half();
        self.pulse2.half();
        self.triangle.half();
        self.noise.half();
    }

    /// The mixer's output now, before the filters: the console's two DAC curves.
    fn mix(&self) -> f64 {
        let p = (self.pulse1.output() + self.pulse2.output()) as f64;
        let pulse = if p == 0.0 { 0.0 } else { 95.88 / (8128.0 / p + 100.0) };
        let (t, n, d) = (self.triangle.output() as f64, self.noise.output() as f64, self.dmc_level as f64);
        let tnd = if t + n + d == 0.0 { 0.0 } else { 159.79 / (1.0 / (t / 8227.0 + n / 12241.0 + d / 22638.0) + 100.0) };
        pulse + tnd
    }

    /// Run `cycles` CPU cycles, making samples as they fall due.
    pub fn run(&mut self, cycles: u32) {
        for _ in 0..cycles {
            // The frame counter, in CPU cycles (NTSC).
            self.frame_cycle += 1;
            match (self.five_step, self.frame_cycle) {
                (_, 7457) | (false, 22371) | (true, 22371) => self.quarter(),
                (_, 14913) => {
                    self.quarter();
                    self.half();
                }
                (false, 29829) | (true, 37281) => {
                    self.quarter();
                    self.half();
                    self.frame_cycle = 0;
                }
                _ => {}
            }
            // The triangle's timer runs at the CPU rate; the others at half.
            self.triangle.clock_timer();
            self.odd = !self.odd;
            if self.odd {
                self.pulse1.clock_timer();
                self.pulse2.clock_timer();
                self.noise.clock_timer();
            }
            self.acc += self.mix();
            self.acc_n += 1;
            self.owed -= 1.0;
            if self.owed <= 0.0 {
                self.owed += self.cycles_per_sample;
                let x = self.acc / self.acc_n as f64;
                self.acc = 0.0;
                self.acc_n = 0;
                let y = self.filters.iter_mut().fold(x, |v, f| f.step(v));
                self.samples.push(y as f32);
            }
        }
    }

    /// The samples made so far (about −1 to 1), emptying the buffer.
    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.samples)
    }

    /// Each channel's level and period now.
    pub fn channels(&self) -> Channels {
        let p = |c: &Pulse| (if c.length == 0 || c.muted() { 0 } else { c.env.output() }, c.period);
        Channels {
            pulse1: p(&self.pulse1),
            pulse2: p(&self.pulse2),
            triangle: (if self.triangle.linear > 0 && self.triangle.length > 0 { 15 } else { 0 }, self.triangle.period),
            noise: (if self.noise.length == 0 { 0 } else { self.noise.env.output() }, self.noise.period),
        }
    }
}

/// The pulse timer period for a frequency (the triangle's is half this, plus
/// a half: it steps 32 times a cycle against the pulse's 16).
pub fn pulse_period(hz: f64) -> u16 {
    ((CPU_HZ / (16.0 * hz)) - 1.0).round().clamp(0.0, 2047.0) as u16
}

pub fn triangle_period(hz: f64) -> u16 {
    ((CPU_HZ / (32.0 * hz)) - 1.0).round().clamp(0.0, 2047.0) as u16
}

/// A MIDI note number's frequency (69 is A4, 440 Hz).
pub fn note_hz(note: i32) -> f64 {
    440.0 * 2f64.powf((note - 69) as f64 / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upward zero crossings a second, after the filters settle.
    fn pitch(samples: &[f32], rate: u32) -> f64 {
        let s = &samples[samples.len() / 2..];
        let crossings = s.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        crossings as f64 * rate as f64 / s.len() as f64
    }

    fn quiet(samples: &[f32]) -> bool {
        samples[samples.len() / 2..].iter().all(|v| v.abs() < 1e-3)
    }

    #[test]
    fn pulse_plays_its_pitch() {
        let mut apu = Apu::new(44100);
        apu.write(0x4015, 0x0F);
        apu.write(0x4000, 0xBF); // 50% duty, halt, constant volume 15
        let p = pulse_period(440.0);
        apu.write(0x4002, (p & 0xFF) as u8);
        apu.write(0x4003, (p >> 8) as u8);
        apu.run(CPU_HZ as u32 / 2);
        let hz = pitch(&apu.take_samples(), 44100);
        assert!((hz - 440.0).abs() < 3.0, "{hz}");
    }

    #[test]
    fn triangle_plays_an_octave_below_the_same_period() {
        let mut apu = Apu::new(44100);
        apu.write(0x4015, 0x04);
        apu.write(0x4008, 0xFF);
        let p = triangle_period(220.0);
        apu.write(0x400A, (p & 0xFF) as u8);
        apu.write(0x400B, (p >> 8) as u8);
        apu.run(CPU_HZ as u32 / 2);
        let hz = pitch(&apu.take_samples(), 44100);
        assert!((hz - 220.0).abs() < 2.0, "{hz}");
    }

    #[test]
    fn the_length_counter_silences_a_note() {
        let mut apu = Apu::new(44100);
        apu.write(0x4015, 0x01);
        apu.write(0x4000, 0x9F); // no halt: the length counter runs
        let p = pulse_period(440.0);
        apu.write(0x4002, (p & 0xFF) as u8);
        apu.write(0x4003, (p >> 8) as u8 | (3 << 3)); // length index 3: 2 half frames
        apu.run(CPU_HZ as u32 / 2);
        assert!(quiet(&apu.take_samples()));
    }

    #[test]
    fn the_envelope_decays_to_silence() {
        let mut apu = Apu::new(44100);
        apu.write(0x4015, 0x08);
        // Envelope (not constant, not looping: bit 5 is both the length
        // halt and the envelope loop), divider period 2; a long length.
        apu.write(0x400C, 0x02);
        apu.write(0x400E, 0x04);
        apu.write(0x400F, 0x08);
        apu.run(CPU_HZ as u32 / 20);
        let early = apu.take_samples().iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        apu.run(CPU_HZ as u32);
        let late = apu.take_samples();
        assert!(early > 0.01, "{early}");
        assert!(quiet(&late));
    }

    #[test]
    fn the_sweep_raises_the_pitch() {
        let mut apu = Apu::new(44100);
        apu.write(0x4015, 0x01);
        apu.write(0x4000, 0xBF);
        apu.write(0x4001, 0x8B); // sweep on, period 0, negate, shift 3: up
        let p = pulse_period(220.0);
        apu.write(0x4002, (p & 0xFF) as u8);
        apu.write(0x4003, (p >> 8) as u8);
        apu.run(CPU_HZ as u32 / 10);
        assert!(apu.channels().pulse1.1 < p / 2, "{}", apu.channels().pulse1.1);
    }

    #[test]
    fn short_noise_repeats_every_93_steps() {
        let mut n = Noise { short: true, ..Noise::default() };
        let start = n.shift;
        let mut seen = 0;
        for i in 1..=200 {
            n.timer = 0;
            n.clock_timer();
            if n.shift == start {
                seen = i;
                break;
            }
        }
        assert_eq!(seen, 93);
    }

    #[test]
    fn the_mixer_is_non_linear() {
        let mut apu = Apu::new(44100);
        // The triangle rests at step 15 (output 0); otherwise its resting
        // level, a constant the output filters remove, adds in.
        apu.triangle.step = 15;
        apu.pulse1.length = 1;
        apu.pulse1.period = 100;
        apu.pulse1.duty = 3;
        apu.pulse1.step = 0;
        apu.pulse1.env.constant = true;
        apu.pulse1.env.volume = 15;
        let one = apu.mix();
        apu.pulse2 = Pulse { second: true, ..apu.pulse1.clone() };
        let two = apu.mix();
        assert!(two < one * 2.0 && two > one * 1.5, "{one} {two}");
    }
}
