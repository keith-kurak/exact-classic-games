import type { Answer, Sources } from './app.contract.d.ts';

export const grants = '';

// No data sources: the rules are in the contracts, the picture in gpu/.
const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
