// Applies this repo's patches to a local Exact checkout: each file in
// patches/exact/, in name order. The checkout is $EXACT2, or ../../exact from
// this repo (where every game's app.json expects it). A patch already in
// place is skipped; one that no longer fits is reported and left alone.
//
//   bun run patch-exact            apply them
//   bun run patch-exact --check    only say which are applied
//   bun run patch-exact --revert   take them back out
import { existsSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dir, '..');
const exact = resolve(process.env.EXACT2 ?? join(root, '..', '..', 'exact'));
const dir = join(root, 'patches', 'exact');
const mode = process.argv.includes('--revert') ? 'revert' : process.argv.includes('--check') ? 'check' : 'apply';

if (!existsSync(join(exact, '.git'))) {
  console.error(`No Exact checkout at ${exact} (set EXACT2 to its path).`);
  process.exit(1);
}

function git(...args: string[]): boolean {
  return Bun.spawnSync(['git', '-C', exact, 'apply', ...args], { stderr: 'pipe' }).exitCode === 0;
}

let failed = false;
for (const name of readdirSync(dir).filter((f) => f.endsWith('.patch')).sort()) {
  const patch = join(dir, name);
  const applied = git('--reverse', '--check', patch);
  const fits = !applied && git('--check', patch);
  if (mode === 'check') {
    console.log(`${applied ? 'applied    ' : fits ? 'not applied' : 'CONFLICTS  '} ${name}`);
    failed ||= !applied && !fits;
  } else if (mode === 'revert') {
    if (applied && git('--reverse', patch)) console.log(`reverted   ${name}`);
    else console.log(`not applied ${name}`);
  } else if (applied) {
    console.log(`already    ${name}`);
  } else if (fits && git(patch)) {
    console.log(`applied    ${name}`);
  } else {
    console.error(`CONFLICTS  ${name}: Exact changed under it; nothing written. Update the patch.`);
    failed = true;
  }
}
if (mode !== 'check' && !failed) console.log(`Exact at ${exact}. Rebuild a game (bun exact.mjs mac) to pick the change up.`);
process.exit(failed ? 1 : 0);
