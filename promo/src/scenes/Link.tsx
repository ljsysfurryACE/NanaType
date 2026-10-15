import React from 'react';
import { Img, interpolate, staticFile, useCurrentFrame } from 'remotion';
import { Backdrop, Card, Sparkle, Title, useSpring, useTyped } from '../components/ui';
import { ScaledSkin } from '../components/ScaledSkin';
import { parseMarkup, plain } from '../markup';
import { C, fMono, fRound, fSans } from '../theme';

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const;

/* ======================= Blivechat ======================= */
const STEPS = ['弹幕汇入历史', '回复弹幕', '@ 艾特观众'];

/** Every functional pixel is captured from NanaUI, using its real buttons,
 * editor and Core. Remotion supplies only the surrounding editorial layout.
 * Generate with `npm run capture:chat` before rendering. */
export const Blivechat: React.FC = () => {
  const f = Math.min(149, useCurrentFrame());
  const step = f < 50 ? 0 : f < 118 ? 1 : 2;
  const frame = String(f).padStart(4, '0');
  return (
    <Backdrop>
      <div style={{ position: 'absolute', left: 100, top: 90, width: 820 }}>
        <Title kicker="BLIVECHAT" title="弹幕联动" sub="观众的话，直接接进你的台词" size={96} />
      </div>
      <div style={{ position: 'absolute', left: 100, top: 400, display: 'flex', flexDirection: 'column', gap: 18 }}>
        {STEPS.map((s, i) => (
          <div key={s} style={{
            fontFamily: fSans, fontWeight: 900, fontSize: 44, padding: '10px 30px', borderRadius: 999, width: 'fit-content',
            background: step === i ? `linear-gradient(90deg, ${C.blue}, ${C.blueLight})` : '#ffffffaa',
            color: step === i ? '#fff' : C.muted, boxShadow: step === i ? `0 6px 18px ${C.blue}55` : 'none',
            transform: `translateX(${step === i ? 18 : 0}px)`,
          }}>{i + 1} · {s}</div>
        ))}
      </div>
      <div style={{ position: 'absolute', left: 100, top: 735, fontFamily: fSans, fontWeight: 700, fontSize: 25, color: C.muted }}>
        展示框 · 实时镜像 · 本地回复
      </div>
      <Img src={staticFile(`chat-parity/display/${frame}.png`)}
        style={{ position: 'absolute', left: 82, top: 775, width: 900 }} />
      <Card style={{ position: 'absolute', left: 1000, top: 70, width: 830, height: 940, overflow: 'hidden' }}>
        <Img src={staticFile(`chat-parity/panel/${frame}.png`)} style={{ width: '100%', height: '100%', objectFit: 'fill' }} />
      </Card>
      <div style={{ position: 'absolute', right: 100, bottom: 24, fontFamily: fSans, fontSize: 23, fontWeight: 700, color: C.muted }}>
        主播互动面板 · 真实应用录制
      </div>
      <Sparkle size={34} color={C.pink} style={{ position: 'absolute', left: 940, top: 60 }} rot={f} />
    </Backdrop>
  );
};

/* ======================= VTS event-driven ======================= */
const EVENTS: { at: number; type: string; arg: string; map?: string }[] = [
  { at: 6, type: 'line_start', arg: 'id: 1' },
  { at: 10, type: 'speaking', arg: 'speaking: true', map: '张嘴同步' },
  { at: 40, type: 'expression', arg: 'name: "smile"', map: '热键 · Smile' },
  { at: 58, type: 'shake', arg: 'amp: 6, ms: 300', map: '热键 · Shake' },
  { at: 84, type: 'speaking', arg: 'speaking: false', map: '闭嘴' },
  { at: 90, type: 'line_end', arg: 'id: 1' },
];

const Model: React.FC<{ mouth: number; smile: boolean; shakeX: number; hotkeyFlash: number }> = ({ mouth, smile, shakeX, hotkeyFlash }) => (
  <svg width={380} height={380} viewBox="0 0 200 200" style={{ transform: `translateX(${shakeX}px)` }}>
    <circle cx="100" cy="104" r="86" fill="#5b96e6" />
    <circle cx="100" cy="112" r="64" fill="#fff0e6" stroke="#fff" strokeWidth="4" />
    {smile ? (
      <>
        <path d="M66 100 Q76 86 86 100" stroke="#1b2f4b" strokeWidth="6" fill="none" strokeLinecap="round" />
        <path d="M114 100 Q124 86 134 100" stroke="#1b2f4b" strokeWidth="6" fill="none" strokeLinecap="round" />
      </>
    ) : (
      <>
        <circle cx="76" cy="98" r="7" fill="#1b2f4b" />
        <circle cx="124" cy="98" r="7" fill="#1b2f4b" />
      </>
    )}
    <circle cx="62" cy="120" r="9" fill="#ff9ab5" opacity={smile ? 0.9 : 0.5} />
    <circle cx="138" cy="120" r="9" fill="#ff9ab5" opacity={smile ? 0.9 : 0.5} />
    <ellipse cx="100" cy="134" rx={10 + mouth * 4} ry={3 + mouth * 12} fill="#c2415c" />
    <polygon points="100,6 108,26 130,30 112,42 118,64 100,52 82,64 88,42 70,30 92,26" fill="#ffd54a" opacity={hotkeyFlash} transform="translate(70 -10) scale(0.5)" />
  </svg>
);

export const VtsEvents: React.FC = () => {
  const f = useCurrentFrame();
  const LINE = parseMarkup('今天也要[rainbow]元气满满[/rainbow]！');
  const n = useTyped(plain(LINE.filter((s) => !s.sticker)), 10, { base_ms: 50, cjk_factor: 1.6 }, 1.1);
  const speaking = f >= 10 && f < 84;
  const mouth = speaking ? 0.5 + 0.5 * Math.sin(f * 1.1) : 0;
  const smile = f >= 40 && f < 100;
  const shakeAt = f - 58;
  const shakeX = shakeAt >= 0 && shakeAt < 10 ? Math.sin(shakeAt * 2.4) * 14 * (1 - shakeAt / 10) : 0;
  const flash = interpolate(f, [40, 46, 64], [0, 1, 0], clamp);
  const win = useSpring(6, { damping: 18 });
  return (
    <Backdrop>
      <div style={{ position: 'absolute', left: 100, top: 90, width: 900 }}>
        <Title kicker="VTUBE STUDIO" title="台词即事件" sub="说话、表情、抖动，自动驱动你的模型" size={96} />
      </div>

      <div style={{ position: 'absolute', left: 100, top: 480 }}>
        <ScaledSkin id="halo" width={760} regionH={130} name="奈奈" spans={LINE} visible={n} showContinue={false} />
      </div>

      {/* event stream */}
      <div style={{ position: 'absolute', left: 100, top: 660, display: 'flex', flexDirection: 'column', gap: 10 }}>
        {EVENTS.map((e, i) => (
          <EventRow key={i} e={e} />
        ))}
      </div>

      {/* VTS window */}
      <div
        style={{
          position: 'absolute', left: 1010, top: 100, width: 810, height: 880, borderRadius: 28, overflow: 'hidden',
          background: 'linear-gradient(160deg,#b9dcff,#f1e3ff 60%,#ffe3ee)', boxShadow: '0 20px 56px #1b2f4b44',
          opacity: win, transform: `scale(${0.94 + 0.06 * win})`,
        }}
      >
        <div style={{ height: 52, background: '#1f2229', color: '#aab', fontFamily: fSans, fontWeight: 700, fontSize: 24, display: 'flex', alignItems: 'center', padding: '0 20px', gap: 10 }}>
          <span style={{ width: 12, height: 12, borderRadius: 6, background: '#48d296' }} /> VTube Studio · 已连接 ws://127.0.0.1:8001
        </div>
        {/* real Live2D footage: promo/tools/live2d-capture (Haru, driven by the same event timeline) */}
        <div style={{ position: 'absolute', left: 0, right: 0, top: 52, bottom: 0, overflow: 'hidden' }}>
          <Img
            src={staticFile(`nana/${String(Math.min(f, 134)).padStart(4, '0')}.png`)}
            style={{ position: 'absolute', left: '50%', top: 10, width: 720, height: 789, marginLeft: -360 }}
          />
        </div>
        <div style={{ position: 'absolute', right: 20, top: 66, fontFamily: fSans, fontSize: 18, color: '#1b2f4b99' }}>
          模型：睡衣呐 · Live2D 离屏录制
        </div>
        <div style={{ position: 'absolute', left: 30, right: 30, bottom: 28, display: 'flex', gap: 14, flexWrap: 'wrap' }}>
          {['Smile', 'Shake', 'Wave', 'Blush'].map((h) => {
            const active = (h === 'Smile' && f >= 40 && f < 60) || (h === 'Shake' && f >= 58 && f < 74);
            return (
              <span
                key={h}
                style={{
                  fontFamily: fMono, fontWeight: 700, fontSize: 26, padding: '8px 22px', borderRadius: 999,
                  background: active ? `linear-gradient(90deg, ${C.blue}, ${C.blueLight})` : '#ffffffcc',
                  color: active ? '#fff' : C.navy, transform: `scale(${active ? 1.1 : 1})`,
                }}
              >
                ⌨ {h}
              </span>
            );
          })}
        </div>
      </div>
    </Backdrop>
  );
};

const EventRow: React.FC<{ e: { at: number; type: string; arg: string; map?: string } }> = ({ e }) => {
  const p = useSpring(e.at, { damping: 14, stiffness: 240 });
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 14, opacity: p, transform: `translateX(${(1 - p) * -50}px)` }}>
      <span
        style={{
          fontFamily: fMono, fontWeight: 700, fontSize: 28, color: '#fff', background: C.navy, borderRadius: 12, padding: '4px 16px', minWidth: 190,
        }}
      >
        {e.type}
      </span>
      <span style={{ fontFamily: fMono, fontWeight: 500, fontSize: 26, color: C.muted, minWidth: 250 }}>{e.arg}</span>
      {e.map && <span style={{ fontFamily: fSans, fontWeight: 900, fontSize: 26, color: C.blue }}>→ {e.map}</span>}
    </div>
  );
};

void fRound;
