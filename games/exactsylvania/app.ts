import type { Answer, Sources } from './app.contract.d.ts';

export const grants = '';

// No data sources: the picture is drawn by gpu/, and the rules are in the contracts.
const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
