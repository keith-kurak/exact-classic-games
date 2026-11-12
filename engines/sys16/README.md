# sys16

A picture processor in the style of Sega's mid-80s arcade boards: System 16's colour and tiles,
and the Super Scaler's sprite zoom (the hardware behind Space Harrier, Hang-On and Out Run).

| Part | What it does |
| --- | --- |
| Screen | 320×224, 15-bit colour (5 bits a channel: `rgb`, `rgb8`) |
| Art | 4 bits a pixel, colour 0 clear, each piece drawn in one 16-colour palette line |
| Sprites | `Sprite::zoom`: any art at any size, sampled a whole source pixel at a time, as the hardware stepped through its ROM, so scaled art keeps hard pixels that crawl as it grows; drawn far to near; `shadow` sprites halve what they cover |
| Backdrop | a colour for every scanline (raster sky gradients) |
| Layers | 8×8 tiles that wrap sideways, with a horizontal scroll for every line (parallax) |
| Ground | a perspective checkerboard drawn a line at a time from the camera's height, the distance travelled and its sideways shift, fading into fog |
| Text | `font_tiles`: the shared bold font with an arcade drop shadow |

```rust
let frame = sys16::render(&sys16::Scene { palettes, images, tiles, backdrop, back, ground, sprites, front, max_sprites: Some(128) });
frame.to_rgba(&mut rgba);   // 320×224×4
```

It is shown on screen by [`nes-screen`](../nes-screen), whose `NesGame::size` takes any frame size.
Exact Space Dodgeball uses it.
