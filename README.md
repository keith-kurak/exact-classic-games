# Exact Classic Games

Small games made with [Exact](../../exact), each in the style of a classic.

| Game | Style | Folder |
| --- | --- | --- |
| **Exactbound** | A 16-bit-era RPG slice: walk Cheery Cheery Village, talk to the Order of the Placid Blue, and fight them in turn-based battles over a GPU aurora background. | [`games/exactbound`](games/exactbound) |
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
```

Or from a game's folder, `bun exact.mjs mac --run`. The first build takes 1–3 minutes;
later builds take seconds. The app stays in the foreground of the terminal, with its log,
until you quit it.

To build without launching, run `bun exact.mjs mac` in the game's folder. The app is at
`games/<game>/target/clients/<id>/<bundle id>/macos/<Name>.app`; the command prints the path.

## Test

```sh
bun run test              # both games
bun run test:exactbound   # 8 tests
bun run test:links        # 5 tests
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
menu (Fire and Heal). **Auto Fight** plays the battle for you; any key takes the controls back.

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
games/
  exactbound/       app.contract, world.contract, battle.contract, gpu/ (aurora shader), assets/
  links-exact86/    app.contract, course.contract, golfer.contract, gpu/ (course shader)
```

In each game, `app.contract` is the view and the game logic, `gpu/` is a Rust crate with
WGSL shaders that draw into a `canvas`, and `CLAUDE.md` has notes for working on it with
Claude Code.

## Assets

Exactbound's music and sound effects in `games/exactbound/assets/` were supplied for
personal use. Check their rights before you publish this repo.
