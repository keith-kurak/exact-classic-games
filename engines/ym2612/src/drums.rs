//! Drum samples for channel 6's DAC, made here rather than recorded: 8-bit
//! unsigned PCM at the rate a sound driver feeds the DAC (a byte every fourth
//! chip sample, about 13.3 kHz), the way Mega Drive drivers played drums.

use crate::CHIP_HZ;

/// Chip samples between DAC writes.
pub const DAC_DIVIDER: u32 = 4;

/// The DAC's sample rate.
pub fn dac_hz() -> f64 {
    CHIP_HZ / DAC_DIVIDER as f64
}

/// A small noise source (a 32-bit xorshift), the same every time.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn pcm(seconds: f32, mut f: impl FnMut(f32) -> f32) -> Vec<u8> {
    let rate = dac_hz() as f32;
    let n = (seconds * rate) as usize;
    (0..n).map(|i| (128.0 + f(i as f32 / rate).clamp(-1.0, 1.0) * 127.0).round() as u8).collect()
}

/// A bass drum: a sine falling from about 160 to 50 Hz, with a click.
pub fn kick() -> Vec<u8> {
    let mut phase = 0.0f32;
    let mut noise = Noise(0x1234_5678);
    let dt = 1.0 / dac_hz() as f32;
    pcm(0.22, |t| {
        let hz = 50.0 + 110.0 * (-t * 30.0).exp();
        phase += hz * dt;
        let body = (phase * std::f32::consts::TAU).sin() * (-t * 11.0).exp();
        let click = if t < 0.004 { noise.next() * 0.6 } else { 0.0 };
        (body * 1.1 + click).clamp(-1.0, 1.0)
    })
}

/// A snare: a short 190 Hz knock under bright noise.
pub fn snare() -> Vec<u8> {
    let mut noise = Noise(0x9E37_79B9);
    let mut last = 0.0;
    pcm(0.24, |t| {
        let n = noise.next();
        let bright = n - last * 0.6;
        last = n;
        let tone = (t * 190.0 * std::f32::consts::TAU).sin() * (-t * 32.0).exp() * 0.7;
        tone + bright * (-t * 15.0).exp() * 0.75
    })
}

/// A closed hi-hat: a tick of high noise.
pub fn hat() -> Vec<u8> {
    let mut noise = Noise(0x0BAD_F00D);
    let mut last = 0.0;
    pcm(0.06, |t| {
        let n = noise.next();
        let high = n - last;
        last = n;
        high * 0.45 * (-t * 70.0).exp()
    })
}

/// An open hi-hat: the same, ringing longer.
pub fn open_hat() -> Vec<u8> {
    let mut noise = Noise(0x0DDB_A11);
    let mut last = 0.0;
    pcm(0.32, |t| {
        let n = noise.next();
        let high = n - last;
        last = n;
        high * 0.4 * (-t * 9.0).exp()
    })
}

/// A hand clap: three quick bursts, then a tail.
pub fn clap() -> Vec<u8> {
    let mut noise = Noise(0xC1A9_0001);
    let mut last = 0.0;
    pcm(0.25, |t| {
        let n = noise.next();
        let band = n - last * 0.2;
        last = n;
        let burst = [0.0, 0.011, 0.022].iter().map(|&s| if t >= s { (-(t - s) * 260.0).exp() } else { 0.0 }).sum::<f32>();
        band * (burst * 0.7 + (-(t - 0.022).max(0.0) * 18.0).exp() * if t > 0.022 { 0.5 } else { 0.0 })
    })
}

/// A tom: a sine falling from `hz` by a fifth.
pub fn tom(hz: f32) -> Vec<u8> {
    let mut phase = 0.0f32;
    let dt = 1.0 / dac_hz() as f32;
    pcm(0.3, move |t| {
        let f = hz * (0.66 + 0.34 * (-t * 14.0).exp());
        phase += f * dt;
        (phase * std::f32::consts::TAU).sin() * (-t * 8.0).exp() * 0.95
    })
}

pub fn low_tom() -> Vec<u8> {
    tom(110.0)
}

pub fn high_tom() -> Vec<u8> {
    tom(170.0)
}

/// A crash cymbal: a long wash of high noise.
pub fn crash() -> Vec<u8> {
    let mut noise = Noise(0xFACE_B00C);
    let mut last = 0.0;
    pcm(1.1, |t| {
        let n = noise.next();
        let high = n - last * 0.9;
        last = n;
        high * 0.45 * (-t * 3.2).exp()
    })
}

/// An orchestra-style hit, for effects: noise and a low chord, falling away.
pub fn boom() -> Vec<u8> {
    let mut noise = Noise(0x5EED_0042);
    pcm(0.7, |t| {
        let chord: f32 = [55.0, 82.5, 110.0].iter().map(|&f| (t * f * std::f32::consts::TAU).sin()).sum::<f32>() / 3.0;
        (chord * 0.8 + noise.next() * 0.5 * (-t * 12.0).exp()) * (-t * 4.5).exp()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drums_have_body_and_end_quiet() {
        for (name, s) in [("kick", kick()), ("snare", snare()), ("hat", hat()), ("clap", clap()), ("crash", crash()), ("tom", low_tom())] {
            let peak = s.iter().map(|&v| (v as i32 - 128).abs()).max().unwrap();
            assert!(peak > 30, "{name} peak {peak}");
            let tail = s[s.len() - 20..].iter().map(|&v| (v as i32 - 128).abs()).max().unwrap();
            assert!(tail < 12, "{name} tail {tail}");
        }
        assert_eq!(kick().len(), (0.22 * dac_hz() as f32) as usize);
    }
}
