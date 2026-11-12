# ym2612

The Yamaha YM2612 (OPN2), the Mega Drive's FM synthesis chip, emulated sample by sample at its
own rate (the NTSC console's 7.67 MHz master clock over 144: about 53 267 samples a second) and
driven the way a sound driver drives it: by writing its registers.

| Part | What it does |
| --- | --- |
| Operators | A sine read through a 256-step log-sine table and a 256-step exponent table, in integer steps as the chip does; 14-bit output. Pitch from the channel's F-number and block, then detune (by key code) and multiple (½, 1–15). |
| Channels | Six, of four operators each; the eight algorithms; operator 1's feedback (the last two outputs, shifted by 0–7) |
| Envelopes | Attack, first decay, sustain level, second decay, release; rate key scaling; ticks every third sample through the chip's increment patterns; total level in 0.75 dB steps |
| SSG-EG | Repeat, alternate, hold and invert, with the 4× decay rate |
| LFO | 8 rates (about 4 to 72 Hz); tremolo (AMS 0–3, per operator) and vibrato (PMS 0–7) |
| Channel 3 | Special mode: operators 1–3 each take their own frequency (`$A8`–`$AE`) |
| Channel 6 | The DAC: 8-bit samples written to `$2A` replace its FM output (`$2B` bit 7) |
| Output | Each channel clipped to 14 bits, cut to the 9-bit DAC with a small crossover step (an approximation of the "ladder effect"), panned left, right or both; a DC blocker, an optional low-pass, and linear resampling to the output rate |

Not emulated: the timers and status register (there is no CPU to read them), CSM key-on, and
the chip's exact pipeline delays between operators.

```rust
use ym2612::{fnum_for, Ym2612, CHIP_HZ};
let mut chip = Ym2612::new(44100);
chip.write(0, 0xB0, 0x07);              // channel 1: algorithm 7
chip.write(0, 0xB4, 0xC0);              // both speakers
for slot in [0x30, 0x34, 0x38, 0x3C] {  // multiple 1; only operator 4 heard
    chip.write(0, slot, 0x01);
    chip.write(0, slot + 0x10, if slot == 0x3C { 0 } else { 127 });
    chip.write(0, slot + 0x20, 0x1F);   // fastest attack
}
let (block, fnum) = fnum_for(440.0);
chip.write(0, 0xA4, (block << 3) | (fnum >> 8) as u8);
chip.write(0, 0xA0, fnum as u8);
chip.write(0, 0x28, 0xF0);              // key on all four operators
chip.run(CHIP_HZ as u32);               // one second
let samples = chip.take_samples();      // interleaved left, right
std::fs::write("a440.wav", ym2612::wav::encode_stereo(&samples, 44100, 1.0)).unwrap();
```

`driver` is a small 60 Hz sound driver on top. Its patches are an algorithm, feedback, four
operators and LFO depths; its instruments add a volume, software vibrato and a transposition.
Songs are tracker text (`C4 . E4 -`, `@1A3`), six channels; with a drum kit, channel 6 plays
DAC samples a byte every fourth chip sample (about 13.3 kHz), as a Z80 driver did.
`driver::render` returns the samples and each row's notes, levels and instruments;
`driver::Script` records raw register writes frame by frame for sound effects, and
`driver::render_writes` plays them. `drums` makes the DAC samples (kick, snare, hats, clap,
toms, crash) from noise and sines.

Exact apps play declared WAV files, so an app renders its music ahead of time and ships the
WAVs; [Exact Drive Soundscapes](../../games/exact-drive-soundscapes) shows how. `cargo test` checks
the tables, pitch, multiples, total level, envelopes, a modulator's harmonics, panning, the
DAC, channel 3's special mode, the LFO, SSG-EG and the driver.
