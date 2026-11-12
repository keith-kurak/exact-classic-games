// Brings the local Exact checkout up to date and puts this repo's patches
// back on it: take the patches out, fast-forward to the upstream branch, apply
// them again. It stops before changing anything when the checkout holds other
// changes of its own or cannot fast-forward, and puts the patches back if the
// pull fails.
//
//   bun run update-exact            update and re-patch
//   bun run update-exact --check    fetch and say how far behind, change nothing
//   bun run update-exact --games    also refresh each game's generated files
//                                   (bun exact.mjs update, then lock)
import { readdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { exact, git, patches, requireCheckout, state, applyAll, revertAll, root } from './exact-checkout';

requireCheckout();
const args = process.argv.slice(2);
const fail = (msg: string): never => { console.error(msg); process.exit(1); };

const branch = git('rev-parse', '--abbrev-ref', 'HEAD').out;
const upstream = git('rev-parse', '--abbrev-ref', '@{u}');
if (!upstream.ok) fail(`Exact's branch ${branch} tracks no upstream; nothing to update from.`);
console.log(`Exact at ${exact}, on ${branch} (tracking ${upstream.out}). Fetching…`);
if (!git('fetch', '--quiet').ok) fail('git fetch failed: check the network or your access to the Exact repo.');
const [ahead, behind] = git('rev-list', '--left-right', '--count', `HEAD...@{u}`).out.split(/\s+/).map(Number);
console.log(`${behind} commit(s) behind${ahead ? `, ${ahead} ahead (local commits)` : ''}.`);

if (args.includes('--check')) {
  for (const p of patches()) console.log(`  ${state(p.path).padEnd(9)} ${p.name}`);
  process.exit(0);
}
if (ahead) fail('The checkout has local commits, so it cannot fast-forward. Rebase or reset it yourself first.');

// Out with the patches; anything still changed is someone's own work.
revertAll();
const dirty = git('status', '--porcelain', '--untracked-files=no').out;
if (dirty) {
  applyAll(false);
  fail(`The checkout holds changes that are not this repo's patches; left as they were:\n${dirty}`);
}

if (behind) {
  const pull = git('merge', '--ff-only', '@{u}');
  if (!pull.ok) { applyAll(false); fail(`The fast-forward failed; patches put back.\n${pull.out}`); }
  console.log(`Updated to ${git('log', '-1', '--format=%h %s').out}`);
} else {
  console.log('Already up to date.');
}

// Back in with the patches.
const bad = applyAll();
if (bad.length) {
  console.error(`\nExact is updated, but ${bad.length} patch(es) no longer fit and are NOT applied: ${bad.join(', ')}.`);
  console.error('Exact may have fixed it upstream (then delete the patch), or the patch needs redoing');
  console.error('against the new code (the update-exact skill in .claude/skills says how).');
}

if (args.includes('--games')) {
  for (const g of readdirSync(join(root, 'games'))) {
    const dir = join(root, 'games', g);
    if (!existsSync(join(dir, 'exact.mjs'))) continue;
    console.log(`\n${g}: bun exact.mjs update, lock`);
    for (const verb of ['update', 'lock']) {
      const r = Bun.spawnSync(['bun', 'exact.mjs', verb], { cwd: dir, stdout: 'pipe', stderr: 'pipe' });
      const out = (r.stdout.toString() + r.stderr.toString()).trim().split('\n').slice(-2).join('\n  ');
      console.log(`  ${verb}: ${r.exitCode === 0 ? 'ok' : 'FAILED'}  ${out}`);
    }
  }
}
console.log('\nRebuild and test the games: bun run test (or one game: bun run test:<game>).');
process.exit(bad.length ? 1 : 0);
