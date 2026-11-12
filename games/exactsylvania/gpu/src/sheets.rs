// The sprite sheets and where each frame is in them. Written by
// tools/art.py --export; edit it when you move frames in a sheet.

use crate::sheets_types::{Frame, Tiles};

pub const HERO: &[Frame] = &[
    Frame { name: "stand", x: 0, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "walk0", x: 16, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "walk1", x: 32, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "walk2", x: 48, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "walk3", x: 64, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "jump", x: 80, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "crouch", x: 96, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "whipback", x: 112, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "whipfore", x: 128, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "cwhipback", x: 144, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "cwhipfore", x: 160, y: 0, w: 16, h: 32, palette: 0 },
    Frame { name: "hurt", x: 176, y: 0, w: 16, h: 32, palette: 0 },
];
pub const ENEMIES: &[Frame] = &[
    Frame { name: "ghoul0", x: 0, y: 0, w: 16, h: 32, palette: 1 },
    Frame { name: "ghoul1", x: 16, y: 0, w: 16, h: 32, palette: 1 },
    Frame { name: "knight0", x: 32, y: 0, w: 16, h: 32, palette: 2 },
    Frame { name: "knight1", x: 48, y: 0, w: 16, h: 32, palette: 2 },
    Frame { name: "bat0", x: 64, y: 0, w: 16, h: 16, palette: 2 },
    Frame { name: "bat1", x: 80, y: 0, w: 16, h: 16, palette: 2 },
];
pub const BOSS: &[Frame] = &[
    Frame { name: "gbat0", x: 0, y: 0, w: 48, h: 24, palette: 3 },
    Frame { name: "gbat1", x: 48, y: 0, w: 48, h: 24, palette: 3 },
];
pub const ITEMS: &[Frame] = &[
    Frame { name: "lash", x: 0, y: 0, w: 40, h: 8, palette: 2 },
    Frame { name: "lashback", x: 40, y: 0, w: 16, h: 16, palette: 2 },
    Frame { name: "dagger", x: 56, y: 0, w: 16, h: 8, palette: 2 },
    Frame { name: "candle0", x: 72, y: 0, w: 8, h: 16, palette: 3 },
    Frame { name: "candle1", x: 80, y: 0, w: 8, h: 16, palette: 3 },
    Frame { name: "heart", x: 88, y: 0, w: 8, h: 8, palette: 3 },
    Frame { name: "bigheart", x: 96, y: 0, w: 16, h: 16, palette: 3 },
    Frame { name: "puff0", x: 112, y: 0, w: 16, h: 16, palette: 3 },
    Frame { name: "puff1", x: 0, y: 16, w: 16, h: 16, palette: 3 },
];
/// Background tiles: map character and palette, 16×16 each, 8 a row.
pub const TILES: Tiles = Tiles { size: 16, per_row: 8, cells: &[
    (' ', 2),
    ('`', 2),
    ('m', 2),
    ('n', 2),
    ('o', 2),
    ('q', 2),
    ('k', 2),
    ('K', 2),
    ('j', 2),
    ('T', 1),
    ('t', 1),
    ('F', 0),
    ('f', 0),
    ('W', 0),
    ('A', 0),
    ('a', 0),
    ('c', 3),
    ('C', 3),
    ('b', 0),
    ('w', 2),
    ('v', 2),
    ('P', 0),
    ('=', 1),
    ('/', 0),
    ('z', 0),
    ('D', 1),
    ('E', 1),
    ('d', 1),
    ('e', 1),
    ('#', 0),
] };
/// HUD tiles, 8×8 in a row, in the HUD palette (3).
pub const HUD: &[&str] = &["bar20", "bar21", "bar22", "bar10", "bar11", "bar12", "heart", "dagger0", "dagger1"];
