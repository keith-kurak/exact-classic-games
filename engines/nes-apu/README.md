# nes-apu

The NES's audio processing unit (the 2A03's APU), emulated cycle by cycle at the NTSC CPU clock
(1.789773 MHz) and driven the way a game drives it: by writing its registers, `$4000`–`$4017`.

| Part | What it does |
| --- | --- |
| Pulse 1 and 2 | Four duty cycles (12.5, 25, 50, 75%), a volume envelope, a sweep unit (pulse 1 negates by ones' complement, pulse 2 by two's), a length counter; periods under 8 or sweep targets past `$7FF` mute |
| Triangle | The 32-step wave, its linear counter and length counter |
| Noise | The 15-bit shift register in long and short (93-step, metallic) modes, the 16 NTSC periods, an envelope and length counter |
| DMC | Its 7-bit output level (`$4011`) for direct sample writes; its sample fetching is not emulated |
| Frame counter | 4- and 5-step sequences: envelopes and the linear counter each quarter frame, lengths and sweeps each half |
| Mixer | The console's non-linear DAC curves, then its output filters: high-pass at 90 and 440 Hz, low-pass at 14 kHz |

```rust
let mut apu = nes_apu::Apu::new(44100);
apu.write(0x4015, 0x01);                 // enable pulse 1
apu.write(0x4000, 0xBF);                 // 50% duty, constant volume 15
let p = nes_apu::pulse_period(440.0);
apu.write(0x4002, (p & 0xFF) as u8);
apu.write(0x4003, (p >> 8) as u8);       // starts the note
apu.run(1_789_773);                      // one second of CPU cycles
let samples: Vec<f32> = apu.take_samples();
std::fs::write("a440.wav", nes_apu::wav::encode(&samples, 44100, 1.0)).unwrap();
```

`driver` is a small 60 Hz sound driver on top, like an NES game's: each frame it writes each
channel's registers from its instrument (a volume envelope, a release, an arpeggio, vibrato, a
hardware sweep) and the song's notes, written as tracker text (`C4 . E4 -`, `@1A3`, drum letters
on the noise channel). `driver::render` returns the samples and each row's notes and volumes;
`driver::render_writes` plays raw register writes, frame by frame, for sound effects.

Exact apps play declared WAV files, so a game renders its music and effects ahead of time and
ships the WAVs; [Exacticom Musicbox](../../games/exacticom-musicbox) shows how. `cargo test` checks pitch,
lengths, envelopes, sweeps, the short noise loop and the mixer.
