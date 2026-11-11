# Exactbound audio

The game's audio is not in git. `bun run exactbound` (or `bun scripts/silent-audio.ts` from
the repo root) writes a silent stand-in for each missing file, so the game builds and plays
silently. To hear it, put your own files here with these names; git ignores them, and the
script never overwrites them.

| File | Plays |
| --- | --- |
| `field.mp3` | On the village field, looped |
| `battle.mp3` | In battle, looped |
| `win.mp3` | When a battle is won |
| `player-attack.wav` | A party member hits |
| `enemy-attack.wav` | A foe hits |
| `enemy-die.wav` | A foe is beaten |
| `fire.wav` | Juniper casts Fire |
| `heal.wav` | Juniper casts Heal |
| `text-blip.wav` | Text types in |
| `confirm.wav` | A menu choice |
