// Shared by patch-exact.ts and update-exact.ts: the local Exact checkout and
// this repo's patches to it (patches/exact/*.patch, applied in name order).
import { existsSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

export const root = resolve(import.meta.dir, '..');
/** $EXACT2, or ../../exact from this repo, where every game's app.json expects it. */
export const exact = resolve(process.env.EXACT2 ?? join(root, '..', '..', 'exact'));
export const patchDir = join(root, 'patches', 'exact');

export function requireCheckout() {
  if (!existsSync(join(exact, '.git'))) {
    console.error(`No Exact checkout at ${exact} (set EXACT2 to its path).`);
    process.exit(1);
  }
}

/** A git command in the checkout: its success and output. */
export function git(...args: string[]): { ok: boolean; out: string } {
  const r = Bun.spawnSync(['git', '-C', exact, ...args], { stdout: 'pipe', stderr: 'pipe' });
  return { ok: r.exitCode === 0, out: (r.stdout.toString() + r.stderr.toString()).trim() };
}

export const patches = () =>
  readdirSync(patchDir).filter((f) => f.endsWith('.patch')).sort().map((name) => ({ name, path: join(patchDir, name) }));

export type State = 'applied' | 'fits' | 'conflicts';
export function state(path: string): State {
  if (git('apply', '--reverse', '--check', path).ok) return 'applied';
  return git('apply', '--check', path).ok ? 'fits' : 'conflicts';
}

/** Apply each patch not yet in place; returns the names that would not fit. */
export function applyAll(log = true): string[] {
  const bad: string[] = [];
  for (const p of patches()) {
    const s = state(p.path);
    if (s === 'fits' && git('apply', p.path).ok) log && console.log(`applied    ${p.name}`);
    else if (s === 'applied') log && console.log(`already    ${p.name}`);
    else { bad.push(p.name); console.error(`CONFLICTS  ${p.name}: Exact changed under it; not applied.`); }
  }
  return bad;
}

/** Take each applied patch back out, last first. */
export function revertAll(log = true) {
  for (const p of [...patches()].reverse()) {
    if (state(p.path) === 'applied' && git('apply', '--reverse', p.path).ok) log && console.log(`reverted   ${p.name}`);
  }
}
