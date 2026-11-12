# Exact Classic Games

Small games made with [Exact](../../exact), each in the style of a classic.

Each game has a README with screenshots, its controls, and how it's made.

| Game | Style | README |
| --- | --- | --- |
| **Exactbound** | A 16-bit-era RPG slice: walk Cheery Cheery Village, talk to the Order of the Placid Blue, and fight them in turn-based battles over a GPU aurora background. | [`games/exactbound`](games/exactbound/README.md) |
| **Exact Gear Solid** | A late-90s stealth game: cross a dim storage room past four patrolling guards, with an overhead camera, a radar of their view cones, and a camera that drops beside you when you press your back to a wall. | [`games/exact-gear-solid`](games/exact-gear-solid/README.md) |
| **Exact Gear Solid NES** | The same storage room on the NES engines, in 3D from behind your shoulder: a ray-cast view cut into NES tiles each frame, a pixel-a-metre minimap of red dots, wall-pressing in profile, Dr. Vale on the radio, and a slow, tense chiptune from the NES sound chip. | [`games/exact-gear-solid-nes`](games/exact-gear-solid-nes/README.md) |
| **Exact Odyssey** | An 8-bit console RPG on a real NES-style picture processor: 2C02 colours, 8×8 CHR tiles, attribute-limited backgrounds, 3-colour sprites. Walk Reedholm with Thunda, Tooth and Chocolat, then fight real-time ATB battles with stagger chains, juggling and party stances. | [`games/exact-odyssey`](games/exact-odyssey/README.md) |
| **Exactsylvania** | A gothic action platformer on the shared NES engine, in the style of the 8-bit castle crawlers. Vesper Holt and her chain whip cross the Gatehouse: a moonlit courtyard of rising ghouls, the castle gate, an entrance hall of bats, knights, candles and stairs, and the Belfry, where a giant bat waits. | [`games/exactsylvania`](games/exactsylvania/README.md) |
| **Links Exact86** | An early-90s PC golf game: hole 1, a dogleg-right par 4 laid out after Pebble Beach's first, ray-marched from a fixed camera behind the ball, with a three-press swing. | [`games/links-exact86`](games/links-exact86/README.md) |
| **Exacticom Musicbox** | A jukebox for the shared NES sound engine: three original chiptunes and seven sound effects from an emulated 2A03, with live channel meters on an NES-drawn screen. | [`games/exacticom-musicbox`](games/exacticom-musicbox/README.md) |
| **Exactulator** | A basic NES emulator on the shared NES engines: open a mapper 0 `.nes` file and play it. Passes nestest to the cycle; plays sound through the shared APU; comes with an original demo cart. | [`games/exactulator`](games/exactulator/README.md) |
| **Exact Space Dodgeball** | A mid-80s Sega arcade look on a new Super Scaler-style engine: fly over a checkerboard in space and dodge giant bouncing dodgeballs and pillars, every one zoomed from a speck to screen-filling. | [`games/exact-space-dodgeball`](games/exact-space-dodgeball/README.md) |

Each game is its own Exact app, with its own Cargo workspace and tests. Their histories
came in with `git subtree`, so `git log -- games/<game>` shows each game's commits.

## What you need

- **macOS 14 or later** with the Xcode command-line tools.
- **Bun 1.4.2 or later** (`bun --version`).
- **Rust**, through [rustup](https://rustup.rs). Each game's `rust-toolchain.toml` pins the version.
- **The Exact checkout** at `../../exact`, next to `exact-projects`:

  ```
  ~/GitHub/
    exact/                       ← the Exact checkout
    exact-projects/
      exact-classic-games/       ← this repo
  ```

  If your checkout is somewhere else, set `EXACT2` to its path. Then, in each game folder,
  run `bun <exact>/scripts/exact.mjs new . --update` to repoint its files.

To find what a native build lacks, run `bun ../../exact/scripts/exact.mjs setup --check`
from this folder.

## Run a game

From this folder:

```sh
bun run exactbound    # build Exactbound and open it
bun run links         # build Links Exact86 and open it
bun run gear          # build Exact Gear Solid and open it
bun run gear-nes      # build Exact Gear Solid NES and open it
bun run odyssey       # build Exact Odyssey and open it
bun run exactsylvania # build Exactsylvania and open it
bun run musicbox      # build Exacticom Musicbox and open it
bun run exactulator   # build Exactulator and open it
bun run dodgeball     # build Exact Space Dodgeball and open it
```

Or from a game's folder, `bun exact.mjs mac --run` (for Exactbound, see [Audio](#audio) first). The first build takes 1–3 minutes;
later builds take seconds. The app stays in the foreground of the terminal, with its log,
until you quit it.

To build without launching, run `bun exact.mjs mac` in the game's folder. The app is at
`games/<game>/target/clients/<id>/<bundle id>/macos/<Name>.app`; the command prints the path.

## Test

```sh
bun run test              # every game
bun run test:exactbound   # 8 tests
bun run test:links        # 5 tests
bun run test:gear         # 10 tests
bun run test:gear-nes     # 7 tests
bun run test:odyssey      # 18 tests
bun run test:exactsylvania # 7 tests
bun run test:musicbox     # 3 tests
bun run test:exactulator  # 3 tests
bun run test:dodgeball    # 3 tests
bun run test:engine       # the NES engines' own tests (picture, sound, emulator)
```

The tests are each game's `app.test.contract`. They build the macOS app and drive it.

## Engines

[`engines/sys16`](engines/sys16) is a picture processor in the style of Sega's System 16 and
Super Scaler boards: 320×224 in 15-bit colour, hardware-style sprite zoom, shadow sprites, raster
backdrops, line-scrolled layers and a perspective checkerboard ground. Exact Space Dodgeball uses
it, shown by `nes-screen` (which takes any frame size).

The NES-style games share five Rust crates:

- [`engines/nes-ppu`](engines/nes-ppu): an NES-style picture processor: the 2C02 palette, 2bpp
  8×8 tiles, metatiles and attribute palettes, OAM sprites, fades, a bold 8×8 font, PNG sheets
  in and out, and `Limits`: which NES rules to keep (`Limits::NES`) or lift (`Limits::MODERN`).
- [`engines/nes-screen`](engines/nes-screen): shows its frames on an Exact GPU surface; a game
  implements `NesGame` (its inputs and its scene).
- [`engines/nes-apu`](engines/nes-apu): the NES sound chip (the 2A03's APU), cycle by cycle:
  pulses, triangle, noise, the frame counter and mixer, with a 60 Hz sound driver and a WAV
  writer. Exacticom Musicbox renders its music with it.
- [`engines/nes-emu`](engines/nes-emu): a basic NES emulator core: the 6502, a real PPU front
  end that draws into nes-ppu's frame, the APU, mapper 0 cartridges and controllers. Exactulator
  runs it.
- [`engines/live-audio`](engines/live-audio): plays samples a module makes live, through
  AudioQueue: a stopgap until Exact can stream audio. Exactulator's sound goes through it.

Exact Odyssey draws its art as text in code; Exactsylvania draws everything from PNG sprite
sheets. Each game's README says how.

## Layout

```
engines/
  nes-ppu/          an NES-style picture processor (Rust): palette, CHR tiles, metatiles, OAM
  nes-screen/       shows an nes-ppu frame on an Exact GPU surface (the NesGame trait)
  nes-apu/          the NES sound chip, a sound driver, WAV output
  nes-emu/          an NES emulator core: 6502, PPU, APU, mapper 0, controllers
  live-audio/       live samples to the speaker (AudioQueue), until Exact streams audio
  sys16/            a Super Scaler-style picture processor: 320x224, sprite zoom, ground, raster
games/
  exactsylvania/    app.contract, level.contract, gpu/ (its NesGame, sheets.rs), assets/sprites/ (PNG sheets), tools/
  exact-space-dodgeball/ field.contract, app.contract, gpu/ (art and scene for sys16)
  exactulator/      app.contract, app.ts (reads the ROM), gpu/ (runs nes-emu), assets/demo.nes, tools/democart.py
  exacticom-musicbox/     app.contract, songs.contract (generated), gpu/ (music.rs, its NesGame), assets/ (rendered WAVs)
  exact-odyssey/    app.contract, field.contract, battle.contract, gpu/ (art.rs, its NesGame), tools/ (art generators)
  exactbound/       app.contract, world.contract, battle.contract, gpu/ (aurora shader), assets/ (audio, not in git)
  exact-gear-solid-nes/ room.contract (the original's), calls.contract, app.contract, gpu/ (the 3D view, minimap, radio, music.rs), assets/ (rendered WAVs)
  exact-gear-solid/ app.contract, room.contract, items.contract, gpu/ (room shader)
  links-exact86/    app.contract, course.contract, golfer.contract, gpu/ (course shader)
```

In each game, `app.contract` is the view and the game logic, `gpu/` is a Rust crate that draws
into a `canvas` (WGSL shaders, or the NES engines), `docs/` holds its README's screenshots, and
`CLAUDE.md` has notes for working on it with Claude Code.

## Audio

Exactbound's music and sound effects are not in this repo. `bun run exactbound` and
`bun run test:exactbound` first run `scripts/silent-audio.ts`, which writes a silent
stand-in for each missing file, so the game builds and plays without sound. To hear it, add
your own files with the names in [`games/exactbound/assets/README.md`](games/exactbound/assets/README.md);
git ignores them. If you build from the game's folder, run `bun scripts/silent-audio.ts`
from the repo root once first.
