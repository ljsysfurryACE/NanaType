// Synthesises an original BGM (no samples, no third-party material) and a sound-effects track.
// Output: public/audio/bgm.wav, public/audio/sfx.wav  (44.1 kHz, 16-bit stereo, 40 s)
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const SR = 44100;
const FPS = 30;
const DUR = 1370 / 30;
const N = Math.round(SR * DUR);
const skins = JSON.parse(readFileSync(join(here, '../src/skins.generated.json'), 'utf8'));

// deterministic noise
let seed = 1234567;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);

const mk = () => [new Float32Array(N), new Float32Array(N)];
const add = (buf, t, samples, gain = 1, pan = 0) => {
  const s0 = Math.floor(t * SR);
  const l = gain * Math.min(1, 1 - pan);
  const r = gain * Math.min(1, 1 + pan);
  for (let i = 0; i < samples.length; i++) {
    const k = s0 + i;
    if (k < 0 || k >= N) continue;
    buf[0][k] += samples[i] * l;
    buf[1][k] += samples[i] * r;
  }
};
const env = (i, n, a = 0.002, r = 0.9) => {
  const t = i / SR;
  const att = Math.min(1, t / a);
  const rel = Math.pow(1 - i / n, r * 3);
  return att * rel;
};
const tone = (f, dur, { wave = 'sine', harm = 0, glide = 0 } = {}) => {
  const n = Math.floor(dur * SR);
  const out = new Float32Array(n);
  let ph = 0;
  for (let i = 0; i < n; i++) {
    const fr = f * (1 + glide * (i / n));
    ph += (2 * Math.PI * fr) / SR;
    let v = Math.sin(ph);
    if (wave === 'tri') v = (2 / Math.PI) * Math.asin(Math.sin(ph));
    if (wave === 'saw') v = 2 * ((ph / (2 * Math.PI)) % 1) - 1;
    if (harm) v += harm * Math.sin(ph * 2) + harm * 0.5 * Math.sin(ph * 3);
    out[i] = v * env(i, n);
  }
  return out;
};
const noise = (dur, lp = 0.2, sweep = 0) => {
  const n = Math.floor(dur * SR);
  const out = new Float32Array(n);
  let y = 0;
  for (let i = 0; i < n; i++) {
    const a = Math.min(0.95, lp + sweep * (i / n));
    y += a * (rnd() * 2 - 1 - y);
    out[i] = y * env(i, n, 0.004, 0.8);
  }
  return out;
};
const kick = () => {
  const n = Math.floor(0.22 * SR);
  const out = new Float32Array(n);
  let ph = 0;
  for (let i = 0; i < n; i++) {
    const f = 42 + 110 * Math.exp(-i / (SR * 0.035));
    ph += (2 * Math.PI * f) / SR;
    out[i] = Math.sin(ph) * Math.exp(-i / (SR * 0.09));
  }
  return out;
};

const mtof = (m) => 440 * Math.pow(2, (m - 69) / 12);

/* ---------------- BGM ---------------- */
const bgm = mk();
const BPM = 132;
const beat = 60 / BPM;
const bar = beat * 4;
const CH = [
  { root: 48, tri: [60, 64, 67] }, // C
  { root: 43, tri: [59, 62, 67] }, // G
  { root: 45, tri: [60, 64, 69] }, // Am
  { root: 41, tri: [60, 65, 69] }, // F
];
const totalBars = Math.ceil(DUR / bar);
for (let b = 0; b < totalBars; b++) {
  const c = CH[b % 4];
  const t0 = b * bar;
  const energy = b < 2 ? 0 : 1;
  // pad
  for (const m of c.tri) add(bgm, t0, tone(mtof(m), bar * 0.98, { wave: 'tri' }), 0.05);
  // hats
  for (let i = 0; i < 8; i++) add(bgm, t0 + i * beat / 2, noise(0.04, 0.9), i % 2 ? 0.05 : 0.03, 0.2);
  if (energy) {
    for (let i = 0; i < 4; i++) {
      add(bgm, t0 + i * beat, kick(), 0.42);
      // bass eighth notes
      add(bgm, t0 + i * beat, tone(mtof(c.root), beat * 0.45, { wave: 'saw' }), 0.07);
      add(bgm, t0 + i * beat + beat / 2, tone(mtof(c.root + (i % 2 ? 7 : 12)), beat * 0.4, { wave: 'saw' }), 0.05);
    }
    for (const i of [1, 3]) add(bgm, t0 + i * beat, noise(0.12, 0.45), 0.13, -0.1);
    // chord stabs on the off-beats
    for (const i of [0.5, 1.5, 2.5, 3.5]) for (const m of c.tri) add(bgm, t0 + i * beat, tone(mtof(m + 12), beat * 0.22, { harm: 0.3 }), 0.035, 0.15);
  }
  // 16th arpeggio lead from bar 4
  if (b >= 4) {
    const arp = [c.tri[0], c.tri[1], c.tri[2], c.tri[1] + 12];
    for (let i = 0; i < 16; i++) {
      const m = arp[i % 4] + (i % 8 >= 4 ? 12 : 0) + 12;
      add(bgm, t0 + i * beat / 4, tone(mtof(m), beat * 0.2, { harm: 0.25 }), 0.045, i % 2 ? 0.3 : -0.3);
    }
  }
}
// fade out the final second
for (let i = 0; i < N; i++) {
  const t = i / SR;
  const g = t > DUR - 1.2 ? Math.max(0, (DUR - t) / 1.2) : 1;
  bgm[0][i] *= g;
  bgm[1][i] *= g;
}

/* ---------------- SFX ---------------- */
const sfx = mk();
const isCjk = (c) => /[⺀-鿿豈-﫿＀-￯　-〿]/.test(c);
const typeTimes = (text, r = {}, speed = 1) => {
  const base = r.base_ms ?? 50;
  const f = r.cjk_factor ?? 2;
  const p = { comma_ms: 120, stop_ms: 260, ellipsis_ms: 300, ...(r.punct ?? {}) };
  let t = 0;
  const out = [];
  for (const c of text) {
    if (c !== ' ') t += (isCjk(c) ? base * f : base) / speed;
    out.push(t / 1000);
    if (/[,，、;；]/.test(c)) t += p.comma_ms / speed;
    else if (/[.。!！?？]/.test(c)) t += p.stop_ms / speed;
  }
  return out;
};
const tick = (t, gain = 0.1) => {
  const f = 1500 + rnd() * 900;
  add(sfx, t, tone(f, 0.03, { wave: 'tri' }), gain, (rnd() - 0.5) * 0.4);
};
const typing = (startFrame, text, reveal, speed, gain) => {
  for (const s of typeTimes(text, reveal, speed)) tick(startFrame / FPS + s, gain);
};
const whoosh = (frame) => add(sfx, frame / FPS - 0.12, noise(0.3, 0.05, 0.5), 0.22);
const pop = (frame, f = 880) => {
  add(sfx, frame / FPS, tone(f, 0.18, { glide: 0.8 }), 0.14);
  add(sfx, frame / FPS + 0.04, tone(f * 1.5, 0.22), 0.07);
};
const click = (frame) => {
  add(sfx, frame / FPS, tone(220, 0.05, { wave: 'tri' }), 0.25);
  add(sfx, frame / FPS, noise(0.03, 0.6), 0.12);
};
const chime = (frame) => {
  [72, 76, 79, 84].forEach((m, i) => add(sfx, frame / FPS + i * 0.07, tone(mtof(m), 0.7, { harm: 0.2 }), 0.09));
};

// scene starts (global frames) — keep in sync with src/Root.tsx
const S = { type: 90, feat: 270, skins: 735, bili: 855, vts: 1005, out: 1140, outro: 1260 };
const BEAT = 75;
[S.type, S.feat, S.feat + BEAT, S.feat + BEAT * 2, S.feat + BEAT * 3, S.feat + BEAT * 4, S.feat + BEAT * 5, S.skins, S.bili, S.vts, S.out, S.outro].forEach(whoosh);

// S1
typing(4, '你打的每一句，都是台词', { base_ms: 50, cjk_factor: 1.4, punct: { comma_ms: 60 } }, 1, 0.12);
pop(58, 520);
chime(76);
// S2
typing(S.type + 14, '大家好～我是奈奈！今天也要元气满满地开播！', { base_ms: 50, cjk_factor: 1.5 }, 1.1, 0.1);
click(S.type + 118);
typing(S.type + 130, '马上就开始啦～记得点关注哦！', { base_ms: 50, cjk_factor: 1.5 }, 1.1, 0.1);
[40, 56, 132].forEach((f) => pop(S.type + f, 700));
// S3
typing(S.feat + 30, '红色、波浪、漢字、抖动', { base_ms: 50, cjk_factor: 1.2 }, 1, 0.09);
[8, 20, 32, 44].forEach((f, i) => pop(S.feat + BEAT + f, 600 + i * 120));
click(S.feat + BEAT * 2 + 12);
click(S.feat + BEAT * 2 + 34);
click(S.feat + BEAT * 2 + 54);
// mirror: ticks on each typed/deleted char, then auto-hide beat
for (let i = 0; i < 9; i++) tick((S.feat + BEAT * 3 + 12 + i * 3) / FPS, 0.1);
[46, 49, 52, 55].forEach((f) => tick((S.feat + BEAT * 3 + f) / FPS, 0.07));
for (let i = 0; i < 4; i++) tick((S.feat + BEAT * 3 + 60 + i * 3) / FPS, 0.1);
pop(S.feat + BEAT * 4 + 20, 800);
pop(S.feat + BEAT * 4 + 41, 900);
pop(S.feat + BEAT * 4 + 60, 700);
typing(S.feat + BEAT * 5 + 6, '这句播完，就会自动淡出～', { base_ms: 50, cjk_factor: 1.5 }, 1.4, 0.1);
click(S.feat + BEAT * 5 + 52);
add(sfx, (S.feat + BEAT * 5 + 62) / FPS, noise(0.3, 0.05, 0.4), 0.12);
// S4: the 18-skin grid pops in one card at a time
for (let i = 0; i < 18; i++) tick((S.skins + 4 + i * 1.8) / FPS, 0.14);
// Blivechat: comments in, reply, mention
[8, 20, 32, 112, 138].forEach((f) => pop(S.bili + f, 720));
[56, 122].forEach((f) => click(S.bili + f));
typing(S.bili + 64, '@阿澈 今天打新游戏哦～', { base_ms: 50, cjk_factor: 1.4 }, 1.2, 0.1);
typing(S.bili + 122, '@路过的猫 自我介绍马上来～', { base_ms: 50, cjk_factor: 1.4 }, 1.4, 0.1);
chime(S.bili + 130);
// VTS events
typing(S.vts + 10, '今天也要元气满满！', { base_ms: 50, cjk_factor: 1.6 }, 1.1, 0.1);
[6, 10, 40, 58, 84, 90].forEach((f, i) => pop(S.vts + f, 560 + i * 60));
// S5
typing(S.out + 30, '透明背景，直接入镜～', { base_ms: 50, cjk_factor: 1.5 }, 1.3, 0.1);
[20, 46, 54, 62].forEach((f) => pop(S.out + f, 700));
// S6
pop(S.outro, 560);
chime(S.outro + 8);

/* ---------------- write ---------------- */
const write = (buf, path, norm = 0.9) => {
  let peak = 0;
  for (let c = 0; c < 2; c++) for (let i = 0; i < N; i++) peak = Math.max(peak, Math.abs(buf[c][i]));
  const g = peak > 0 ? norm / peak : 1;
  const data = Buffer.alloc(N * 4);
  for (let i = 0; i < N; i++) {
    data.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(buf[0][i] * g * 32767))), i * 4);
    data.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(buf[1][i] * g * 32767))), i * 4 + 2);
  }
  const h = Buffer.alloc(44);
  h.write('RIFF', 0); h.writeUInt32LE(36 + data.length, 4); h.write('WAVEfmt ', 8);
  h.writeUInt32LE(16, 16); h.writeUInt16LE(1, 20); h.writeUInt16LE(2, 22); h.writeUInt32LE(SR, 24);
  h.writeUInt32LE(SR * 4, 28); h.writeUInt16LE(4, 32); h.writeUInt16LE(16, 34); h.write('data', 36); h.writeUInt32LE(data.length, 40);
  writeFileSync(path, Buffer.concat([h, data]));
};
const out = join(here, '../public/audio');
mkdirSync(out, { recursive: true });
write(bgm, join(out, 'bgm.wav'), 0.8);
write(sfx, join(out, 'sfx.wav'), 0.9);
console.log('audio written');
