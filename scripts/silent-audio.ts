// Exactbound's audio is not in git. Its contract reads every file it declares
// at build time, so this writes a short silent stand-in for each one that is
// missing. Files already there (your own) are never touched.
import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const dir = join(import.meta.dir, '..', 'games', 'exactbound', 'assets');
const music = ['field.mp3', 'battle.mp3', 'win.mp3'];
const sounds = [
  'player-attack.wav', 'enemy-attack.wav', 'enemy-die.wav', 'fire.wav',
  'heal.wav', 'text-blip.wav', 'confirm.wav',
];

// 0.1 s of 16-bit mono PCM silence at 22.05 kHz.
function silentWav(): Uint8Array {
  const rate = 22050;
  const samples = rate / 10;
  const data = samples * 2;
  const buf = new DataView(new ArrayBuffer(44 + data));
  const tag = (at: number, s: string) => [...s].forEach((c, i) => buf.setUint8(at + i, c.charCodeAt(0)));
  tag(0, 'RIFF'); buf.setUint32(4, 36 + data, true); tag(8, 'WAVE');
  tag(12, 'fmt '); buf.setUint32(16, 16, true); buf.setUint16(20, 1, true); buf.setUint16(22, 1, true);
  buf.setUint32(24, rate, true); buf.setUint32(28, rate * 2, true); buf.setUint16(32, 2, true); buf.setUint16(34, 16, true);
  tag(36, 'data'); buf.setUint32(40, data, true);
  return new Uint8Array(buf.buffer);
}

// About 1 s of MPEG-1 Layer III silence: 128 kbps, 44.1 kHz, mono frames whose
// side information is all zero, so every frame decodes to silence.
function silentMp3(): Uint8Array {
  const frame = new Uint8Array(417);
  frame.set([0xff, 0xfb, 0x90, 0xc4]);
  const out = new Uint8Array(frame.length * 38);
  for (let i = 0; i < 38; i++) out.set(frame, i * frame.length);
  return out;
}

let wrote = 0;
for (const name of [...music, ...sounds]) {
  const path = join(dir, name);
  if (existsSync(path)) continue;
  writeFileSync(path, name.endsWith('.mp3') ? silentMp3() : silentWav());
  wrote++;
}
if (wrote > 0) console.log(`silent-audio: wrote ${wrote} silent stand-in(s) in games/exactbound/assets`);
