# Exact Classic Games

Small games made with [Exact](../../exact), each in the style of a classic.

| Game | Style | Folder |
| --- | --- | --- |
| **Exactbound** | A 16-bit-era RPG slice: walk Cheery Cheery Village, talk to the Order of the Placid Blue, and fight them in turn-based battles over a GPU aurora background. | [`games/exactbound`](games/exactbound) |
| **Exact Gear Solid** | A late-90s stealth game: cross a dim storage room past four patrolling guards, with an overhead camera, a radar of their view cones, and a camera that drops beside you when you press your back to a wall. | [`games/exact-gear-solid`](games/exact-gear-solid) |
| **Exact Odyssey** | An 8-bit console RPG on a real NES-style picture processor: 2C02 colours, 8×8 CHR tiles, attribute-limited backgrounds, 3-colour sprites. Walk Reedholm with Thunda, Tooth and Chocolat, then fight real-time ATB battles with stagger chains, juggling and party stances. | [`games/exact-odyssey`](games/exact-odyssey) |
| **Links Exact86** | An early-90s PC golf game: hole 1, a dogleg-right par 4 laid out after Pebble Beach's first, ray-marched from a fixed camera behind the ball, with a three-press swing. | [`games/links-exact86`](games/links-exact86) |

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
bun run odyssey       # build Exact Odyssey and open it
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
bun run test:odyssey      # 12 tests
bun run test:engine       # the NES picture processor's own tests
```

The tests are each game's `app.test.contract`. They build the macOS app and drive it.

## Controls

### Exactbound

| Key | Action |
| --- | --- |
| Arrows or WASD | Walk (hold two keys to walk diagonally) |
| Space, Enter or Z | Talk, go on, choose |
| Escape or X | Back |
| Up / Down | Move through battle menus |
| Left / Right | Choose a target |

Every villager in a blue T-shirt starts a battle after they talk. Juniper has a Magic
menu (Fire and Heal). **Auto Fight** plays the battle for you; Escape or X takes the controls back.

### Exact Gear Solid

| Key | Action |
| --- | --- |
| Arrows or WASD | Move (hold two keys to move diagonally) |
| Push into a wall | After a quarter second, press your back to it; the camera drops beside you |
| Along the wall | Slide; you stop at its corner, so you can look round it |
| Away from the wall | Step off it |
| Q | Items (the game pauses): ← → to choose, Q to equip, Escape to close |
| Space | Go on in a radio call; try again, or play again |

Items: **CIGS** (light up, and your medical officer calls to scold you) and **C.BOX** (hide
inside; a guard notices only a box that moves while it watches).

Reach the exit at the top right of the radar. A guard sees 6 m ahead, 30° either side, unless
a container is in the way. A question mark means a guard has half-seen you; stay in its cone
for about 0.7 s and you are spotted. Leaning round a corner, the wall hides you until a guard is
near its edge. At each waypoint a guard stops and slowly looks both ways.

### Exact Odyssey

| Key | Action |
| --- | --- |
| Arrows or WASD | Walk, tile by tile |
| Space, Enter or Z | Talk; choose in battle |
| Escape or X | Back, when choosing a target |

Battles run in real time: gauges fill and enemies attack while your menu is open; only one
action plays at a time, the next right after. Each member's ATB gauge has two segments, one
ability each. You command Thunda, the leader. The chooser sits bottom left (it goes away while her commands
wait or play out), with her ATB bar above it; choose an ability and its
name appears over a segment. Choose a second, then a target (the chooser becomes a target list),
to commit both; **Auto** fills both by her role and goes straight to the target list. Committed,
the commands stay on the bar until it fills and she acts; press Space or Enter first and she acts
at once with the segments she has, dropping the rest. **Right** commits a single queued ability.
Each segment is a block: it fills, turns solid once charged, and is gone from the bar as its
command finishes. Escape takes back a slot. **Item** commits a Potion, which heals the whole
party, or a Revive, which brings a fallen ally back at half HP. Tooth
and Chocolat fill theirs by role (shown in yellow) and act. Press **Left** for the stance list, in the chooser,
free at any time, even mid-attack, where a shift cancels the party's commands; a "Stance Shift!" banner sweeps across the screen. The stances: **Assault** (all Blast), **Rush** (Strike, Strike, Blast), **Balance** and
**Bulwark**. Only Strike attacks; Blast throws Fire and Ice, which build an enemy's chain fast; Mend
cures and heals, and casts Raise by itself when an ally has fallen; Guard can Taunt and Steel. The chain drains over time, slower just after a Strike hit. The stagger panel
at the top left follows the enemy you last targeted. Each attacker leaps in beside its target, weapon out. Staggered, an enemy takes far more damage (each spell on it keeps the stagger going a little longer),
and Strike hits launch it: upside down in the air, it can't act. Each enemy's HP shows as a green bar
beneath it. A win shows your time and a 1–5 star rank (five stars finds a Potion) while the party
strikes victory poses, and the party
is healed in full after every fight.

The picture comes from [`engines/nes-ppu`](engines/nes-ppu). To write the game's pattern tables
as PNG sprite sheets and two sample frames to `games/exact-odyssey/assets/sheets/`:

```sh
cd games/exact-odyssey && cargo test -p exact-odyssey-gpu sheets -- --ignored
```

### Links Exact86

| Key | Action |
| --- | --- |
| Left / Right | Aim |
| Up / Down | Change club |
| Space (3 times) | Start the backswing, set the power, then press at the white line for accuracy |
| T | Top view of the hole |
| G | Take a gimme (2 yards or less, on the green) |

An early third press hooks the ball and a late one slices it. Past 100% power, the accuracy
gets touchier.

## Layout

```
engines/
  nes-ppu/          an NES-style picture processor (Rust), shared by NES-style games
games/
  exact-odyssey/    app.contract, field.contract, battle.contract, gpu/ (art.rs, the ppu surface)
  exactbound/       app.contract, world.contract, battle.contract, gpu/ (aurora shader), assets/ (audio, not in git)
  exact-gear-solid/ app.contract, room.contract, items.contract, gpu/ (room shader)
  links-exact86/    app.contract, course.contract, golfer.contract, gpu/ (course shader)
```

In each game, `app.contract` is the view and the game logic, `gpu/` is a Rust crate with
WGSL shaders that draw into a `canvas`, and `CLAUDE.md` has notes for working on it with
Claude Code.

## Audio

Exactbound's music and sound effects are not in this repo. `bun run exactbound` and
`bun run test:exactbound` first run `scripts/silent-audio.ts`, which writes a silent
stand-in for each missing file, so the game builds and plays without sound. To hear it, add
your own files with the names in [`games/exactbound/assets/README.md`](games/exactbound/assets/README.md);
git ignores them. If you build from the game's folder, run `bun scripts/silent-audio.ts`
from the repo root once first.
