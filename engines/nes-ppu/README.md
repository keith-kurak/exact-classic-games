# nes-ppu

An NES-style picture processor for Exact games. It keeps the limits that make the
look (palette, 2bpp tiles, attributes, 3-colour sprites, the 8-per-line limit) and relaxes the
ones that only constrain content (tile counts, background flips):

- **Colours**: the 64-entry 2C02 palette, by index (`NES_PALETTE`).
- **Tiles**: 8×8, 2 bits a pixel, in pattern tables (background, sprites). Identical tiles are
  stored once. `to_chr()` gives real CHR-ROM bytes. Unlike the NES, tables may exceed 256 tiles
  (indices are 16-bit; `fits_nes()` says whether one would fit a cartridge).
- **Palettes**: 4 background + 4 sprite palettes of 3 colours, one shared backdrop colour.
- **Backgrounds**: 16×16 metatiles, one palette each (the attribute-table rule), scrolled.
  Unlike the NES, a background cell may be flipped (`Cell::flip`), so art can be reused.
- **Sprites**: 8×8 OAM entries with palette, flips and behind-background bits; colour 0 is
  transparent; at most 64 a frame and 8 a scanline, with OAM rotation so extras flicker.
- **Fades**: every colour steps down 0x10 a level, as NES games faded.
- **Output**: a 256×240 frame of palette indices, `to_rgba()` for display, and `png::sheet`
  to write a pattern table as a sprite-sheet PNG.

Art is written as text, one character a pixel (`.` or `0`–`3`), and parsed with `parse_art`.
A game's GPU module owns its art and turns its own scene description into a `Scene`;
see `games/exact-odyssey/gpu/src/lib.rs`.

```sh
cargo test   # CHR encoding, tile dedupe, the 8-per-line limit, fades
```
