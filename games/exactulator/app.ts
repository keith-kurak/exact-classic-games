import type { Answer, Result, Sources } from './app.contract.d.ts';

// Picked files are copied under app:/tmp/picked/; read them there.
export const grants = 'fs.read app:/tmp/picked';

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

/** Bytes as base64, for the GPU module's ROM input. */
function base64(bytes: Uint8Array): string {
  let out = '';
  for (let i = 0; i < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | ((bytes[i + 1] ?? 0) << 8) | (bytes[i + 2] ?? 0);
    out += ALPHABET[(n >> 18) & 63] + ALPHABET[(n >> 12) & 63];
    out += i + 1 < bytes.length ? ALPHABET[(n >> 6) & 63] : '=';
    out += i + 2 < bytes.length ? ALPHABET[n & 63] : '=';
  }
  return out;
}

const sources: Sources = {
  // A ROM file the person picked: its bytes as base64, or why it can't be used.
  romFile: async ([path], _store, storage): Promise<Result<'romFile'>> => {
    if (!path) return { data: '', error: '' };
    const bytes = new Uint8Array(await storage.fs.readFile(String(path)));
    if (bytes.length > 1024 * 1024) return { data: '', error: 'That file is over 1 MB: too big for a mapper 0 cartridge.' };
    if (bytes[0] !== 0x4e || bytes[1] !== 0x45 || bytes[2] !== 0x53 || bytes[3] !== 0x1a) {
      return { data: '', error: 'That is not an NES ROM (an iNES .nes file).' };
    }
    return { data: base64(bytes), error: '' };
  },
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
