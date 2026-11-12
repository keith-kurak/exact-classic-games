# nes-emu

A basic NES emulator core, sharing [`nes-ppu`](../nes-ppu)'s frame and palette.

```rust
let mut nes = nes_emu::Nes::new(&std::fs::read("game.nes")?)?;   // iNES, mapper 0
nes.set_buttons(0, nes_emu::button::START);
nes.run_frame();                                                 // to the next vertical blank
nes.frame().to_rgba(&mut rgba);                                  // nes_ppu::Frame
```

- `cpu`: the 6502, official and common unofficial opcodes, cycle counts, NMI, OAM DMA stalls.
- `ppu`: registers `$2000`–`$2007` (Loopy's v/t/x/w), name tables with mirroring, palettes,
  OAM, vertical blank and NMI; each visible line drawn at dot 256 from the name, attribute and
  pattern tables, with 8×8 or 8×16 sprites (8 a line, in OAM order), sprite-0 hit and the
  left-column and grayscale masks.
- `cart`: iNES files with mapper 0 (NROM).
- Controllers through `$4016`/`$4017`. The APU's registers are ignored: no sound yet.

`cargo test` runs nestest's automated CPU test to the cycle: point `NESTEST_ROM` at nestest.nes
(it is not kept in this repo), or keep it at `~/Downloads/nestest.nes`.
`NES_ROM=x.nes NES_PNG=out.png cargo test screenshot -- --ignored` writes a ROM's screen.
