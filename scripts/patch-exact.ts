// Applies this repo's patches to the local Exact checkout (see
// patches/exact/README.md). A patch already in place is skipped; one that no
// longer fits is reported and left alone.
//
//   bun run patch-exact            apply them
//   bun run patch-exact --check    only say which are applied
//   bun run patch-exact --revert   take them back out
import { exact, patches, requireCheckout, state, applyAll, revertAll } from './exact-checkout';

requireCheckout();
const args = process.argv.slice(2);
if (args.includes('--check')) {
  let bad = false;
  for (const p of patches()) {
    const s = state(p.path);
    console.log(`${s === 'applied' ? 'applied    ' : s === 'fits' ? 'not applied' : 'CONFLICTS  '} ${p.name}`);
    bad ||= s === 'conflicts';
  }
  process.exit(bad ? 1 : 0);
}
if (args.includes('--revert')) {
  revertAll();
  process.exit(0);
}
const bad = applyAll();
if (!bad.length) console.log(`Exact at ${exact}. Rebuild a game (bun exact.mjs mac) to pick the change up.`);
process.exit(bad.length ? 1 : 0);
