# Patches to Exact

Fixes these games need in Exact itself, kept here so any local Exact checkout can take them.
Apply them from the repo root:

```sh
bun run patch-exact            # apply each patch not yet applied
bun run patch-exact --check    # say which are applied
bun run patch-exact --revert   # take them back out
```

The script finds the checkout at `../../exact` (or `$EXACT2`). It skips a patch that is
already applied, and it reports and skips one that no longer fits a newer Exact. After a
patch, rebuild a game (`bun exact.mjs mac`) to pick the change up.

| Patch | What it fixes |
| --- | --- |
| `gapless-audio-loop.patch` | `audio … loop=true` on macOS and iOS left a short gap each time a track came round: at its end the player sought back to the start. Now the player is an `AVQueuePlayer`, and a looping item has a copy of itself queued behind it, so playback runs straight on into the copy. The jump still reports `seeking`, `seeked` and `timeupdate`, as before. Used by the jukeboxes' looping songs. |

To make a new patch, change the Exact checkout, then save its diff here:
`git -C ../../exact diff -- <files> > patches/exact/<name>.patch`.
