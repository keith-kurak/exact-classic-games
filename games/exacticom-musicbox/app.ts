import type { Answer, Sources } from './app.contract.d.ts';

export const grants = '';

// No data sources: the music is rendered by gpu/ (nes-apu) ahead of time, and
// the picture is drawn by gpu/ (nes-ppu).
const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
