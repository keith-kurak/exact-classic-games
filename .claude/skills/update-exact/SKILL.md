---
name: update-exact
description: Keep the local Exact checkout (../../exact) up to date and patched with this repo's fixes in patches/exact. Use when asked to update, pull or upgrade Exact, to check or apply the Exact patches, when a patch no longer applies after an update, or to make a new patch to Exact for these games.
---

# Keeping local Exact updated and patched

The games build against the Exact checkout at `../../exact` from this repo (or `$EXACT2`).
Fixes the games need in Exact live here as patch files in `patches/exact/` (listed in
`patches/exact/README.md`), applied in name order. Two scripts drive it, from the repo root:

| Command | What it does |
| --- | --- |
| `bun run update-exact --check` | Fetch; say how far behind Exact is and each patch's state. Changes nothing. |
| `bun run update-exact` | Take the patches out, fast-forward Exact to its upstream branch, apply the patches again |
| `bun run update-exact --games` | The same, then `bun exact.mjs update` and `lock` in every game |
| `bun run patch-exact` | Apply any patch not yet applied |
| `bun run patch-exact --check` / `--revert` | Each patch's state / take them all out |

`update-exact` stops without changing anything when Exact has local commits or changes that are
not these patches, and puts the patches back if the fast-forward fails. Never discard someone's
own changes in the Exact checkout to get past that: report them and ask.

## Updating

1. `bun run update-exact --check`, and tell the person how many commits Exact is behind. A large
   update can change the contract language or the build: confirm before pulling.
2. `bun run update-exact --games`.
3. Rebuild and test the games: `bun run test`, or `bun run test:<game>` for one. Fix what the new
   Exact broke in the games (record Exact doc gaps with `bun exact.mjs feedback note`).
4. Commit any game changes the refresh made (`Cargo.lock`, generated files) in this repo.

## When a patch no longer fits

The script reports `CONFLICTS <name>` and leaves that patch out; Exact is updated either way.

1. First see whether upstream fixed it: read the patch's entry in `patches/exact/README.md` and
   the current Exact code it touches. If Exact now does the same thing, delete the patch file
   and its README row.
2. Otherwise redo it against the new code: `git -C ../../exact apply --3way patches/exact/<name>.patch`
   applies what it can and leaves conflict markers; resolve them in the Exact files (keep the
   patch's intent, adapt to the new code). Then `git -C ../../exact reset -q` to unstage.
3. Build and test a game that depends on it (the README row names one), then save the patch
   again, as the diff of just its files:
   `git -C ../../exact diff -- <files> > patches/exact/<name>.patch`.
   With more than one patch touching the same file, revert the later ones first
   (`bun run patch-exact --revert`, then apply the earlier ones), so each file holds one diff.
4. `bun run patch-exact --check` should say `applied`. Commit the patch in this repo.

## Making a new patch

1. Change the Exact checkout; build and test the game that needs it.
2. Save the diff of only the files changed:
   `git -C ../../exact diff -- <files> > patches/exact/<short-name>.patch`.
3. Add a row to `patches/exact/README.md`: what it fixes, how, and which game uses it.
4. Check it from clean: `bun run patch-exact --revert`, then `bun run patch-exact`, which
   should apply it. Commit the patch and README in this repo.

Patches are local fixes. Sending one upstream (a PR to expo/exact) is the person's call: offer,
don't do it unasked.
