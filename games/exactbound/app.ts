import type { Answer, Sources } from './app.contract.d.ts';

export const grants = '';

// No data sources yet: the battle background is drawn by gpu/.
const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
