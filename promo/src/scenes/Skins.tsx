import React from 'react';
import { Img, interpolate, Sequence, staticFile, useCurrentFrame } from 'remotion';
import { Backdrop, Chip, Sparkle, Title, useSpring, useTyped } from '../components/ui';
import { ScaledSkin } from '../components/ScaledSkin';
import { regionOf, SKINS } from '../components/SkinCard';
import { parseMarkup, plain } from '../markup';
import { C, fMono, fSans } from '../theme';

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const;
const LINE = parseMarkup('今天也请多关照！');
const LINE_T = plain(LINE);

export const MONTAGE = ['halo', 'night', 'bubble', 'adv', 'sakura', 'terminal', 'comic', 'rpg', 'parchment', 'pixel', 'ink', 'livechat', 'vanilla', 'subtitle'];
export const ALL = [
  'halo', 'night', 'parchment', 'adv', 'rpg', 'vanilla',
  'bubble', 'subtitle', 'livechat', 'comic', 'terminal', 'pixel',
  'sakura', 'ink', 'echo_vanilla', 'echo_glass', 'echo_bubble', 'echo_void',
];
export const MONTAGE_STEP = 10;
export const GRID_LEN = 120;

const SkinFlash: React.FC<{ id: string; index: number }> = ({ id, index }) => {
  const f = useCurrentFrame();
  const s = SKINS[id];
  const [rw] = regionOf(id);
  const n = useTyped(LINE_T, 1, s.dialogue?.reveal ?? {}, 3.6);
  const p = useSpring(0, { damping: 14, stiffness: 260 });
  const accent = s.chrome?.accent ?? C.blue;
  const width = Math.min(1500, rw * 1.9);
  return (
    <Backdrop bg="radial-gradient(1000px 600px at 50% 45%, #33466f, #1a2233 72%)" sparkles={false}>
      <div style={{ position: 'absolute', left: 0, right: 0, top: 300, display: 'flex', justifyContent: 'center' }}>
        <div style={{ transform: `scale(${0.88 + 0.12 * p}) translateY(${(1 - p) * 30}px)`, opacity: Math.min(1, p * 1.5) }}>
          <ScaledSkin id={id} width={width} name="奈奈" spans={LINE} visible={n} motionFrame={f + index * 7} />
        </div>
      </div>
      <div style={{ position: 'absolute', left: 90, bottom: 90, display: 'flex', alignItems: 'baseline', gap: 22, opacity: p }}>
        <span style={{ fontFamily: fSans, fontWeight: 900, fontSize: 104, color: '#fff', letterSpacing: -1 }}>
          {s.meta.name['en-US']}
        </span>
        <span style={{ fontFamily: fSans, fontWeight: 700, fontSize: 44, color: accent }}>{s.meta.name['zh-CN']}</span>
      </div>
      <div style={{ position: 'absolute', right: 90, bottom: 110, display: 'flex', gap: 12, opacity: p }}>
        {(s.meta.tags as string[]).map((t) => (
          <span key={t} style={{ fontFamily: fMono, fontWeight: 700, fontSize: 28, color: '#fff', background: '#ffffff1f', borderRadius: 999, padding: '4px 18px' }}>
            #{t}
          </span>
        ))}
      </div>
      <div style={{ position: 'absolute', right: 90, top: 70, fontFamily: fMono, fontWeight: 700, fontSize: 36, color: '#ffffff99' }}>
        {String(index + 1).padStart(2, '0')} / {MONTAGE.length}
      </div>
      <div style={{ position: 'absolute', left: 90, top: 70, fontFamily: fMono, fontWeight: 700, fontSize: 30, color: C.sky }}>✦ SKINS</div>
    </Backdrop>
  );
};

const Montage: React.FC = () => (
  <>
    {MONTAGE.map((id, i) => (
      <Sequence key={id} from={i * MONTAGE_STEP} durationInFrames={MONTAGE_STEP}>
        <SkinFlash id={id} index={i} />
      </Sequence>
    ))}
  </>
);

const ECHO = [
  { id: 'echo_vanilla', label: 'Echo-Live 原版' },
  { id: 'echo_glass', label: 'Echo-Live 玻璃' },
  { id: 'echo_bubble', label: 'Echo-Live 气泡' },
  { id: 'echo_void', label: 'Echo-Live 虚空' },
];
const ECHO_LINE = parseMarkup('还原经典的 Echo-Live 主题。');
const ECHO_T = plain(ECHO_LINE);
const EchoCell: React.FC<{ id: string; label: string; i: number }> = ({ id, label, i }) => {
  const f = useCurrentFrame();
  const p = useSpring(3 + i * 4);
  const n = useTyped(ECHO_T, 8 + i * 4, SKINS[id].dialogue?.reveal ?? {}, 1.5);
  return (
    <div style={{ opacity: p, transform: `translateY(${(1 - p) * 60}px) scale(${0.9 + 0.1 * p})` }}>
      <div style={{ position: 'relative', width: 800, height: 150, borderRadius: 18, overflow: 'hidden', background: 'linear-gradient(135deg,#5f7fb4,#a8c4ea)' }}>
        <ScaledSkin id={id} width={800} name="奈奈" spans={ECHO_LINE} visible={n} showContinue={false} motionFrame={f} />
      </div>
      <div style={{ fontFamily: fSans, fontWeight: 900, fontSize: 36, color: '#fff', marginTop: 12 }}>{label}</div>
    </div>
  );
};
const Echo: React.FC = () => (
  <Backdrop bg="linear-gradient(135deg, #1a2233, #2c3a5c)">
    <div style={{ position: 'absolute', left: 100, top: 70 }}>
      <Title kicker="ECHO-LIVE" title="Echo-Live 四件套复刻" sub="从原版到虚空，一套不少" dark size={78} />
    </div>
    <div style={{ position: 'absolute', left: 100, top: 400, display: 'grid', gridTemplateColumns: '800px 800px', gap: '40px 60px' }}>
      {ECHO.map((e, i) => (
        <EchoCell key={e.id} id={e.id} label={e.label} i={i} />
      ))}
    </div>
  </Backdrop>
);

const GRID_LINE = parseMarkup('今天也请多关照！');
const GridCell: React.FC<{ id: string; i: number }> = ({ id, i }) => {
  const p = useSpring(Math.round(i * 1.8), { damping: 13, stiffness: 240 });
  const [rw] = regionOf(id);
  const s = SKINS[id];
  return (
    <div style={{ opacity: p, transform: `scale(${0.6 + 0.4 * p})` }}>
      <div
        style={{
          width: 290, height: 150, borderRadius: 16, background: '#0c0e14', overflow: 'hidden', position: 'relative',
          boxShadow: '0 6px 18px #00000044', display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}
      >
        {/* real NanaType offscreen render (app/examples/snapshots.rs) */}
        <Img
          src={staticFile(`real/nanatype.${id}/display.png`)}
          style={{ width: rw >= 1000 ? 380 : 330, maxWidth: 'none', marginLeft: rw >= 1000 ? -10 : 0 }}
        />
      </div>
      <div style={{ fontFamily: fSans, fontWeight: 700, fontSize: 24, color: '#fff', textAlign: 'center', marginTop: 6 }}>{s.meta.name['zh-CN']}</div>
    </div>
  );
};
const Grid: React.FC = () => {
  const f = useCurrentFrame();
  const count = Math.round(interpolate(f, [4, 40], [0, 18], clamp));
  const sub = useSpring(50);
  return (
    <Backdrop bg="linear-gradient(135deg, #1a2233, #2c3a5c)">
      <div style={{ position: 'absolute', left: 90, top: 40, display: 'flex', alignItems: 'flex-end', gap: 28 }}>
        <span style={{ fontFamily: fSans, fontWeight: 900, fontSize: 190, lineHeight: 1, color: '#fff' }}>{count}</span>
        <span style={{ fontFamily: fSans, fontWeight: 900, fontSize: 72, color: C.sky, paddingBottom: 18 }}>套内置皮肤</span>
      </div>
      <div style={{ position: 'absolute', left: 55, top: 270, display: 'grid', gridTemplateColumns: 'repeat(6, 290px)', gap: '18px 14px' }}>
        {ALL.map((id, i) => (
          <GridCell key={id} id={id} i={i} />
        ))}
      </div>
      <div style={{ position: 'absolute', left: 90, right: 90, bottom: 44, display: 'flex', gap: 20, justifyContent: 'center', opacity: sub }}>
        <Chip delay={52} size={34}>.ntskin 导入导出</Chip>
        <Chip delay={58} size={34}>热重载</Chip>
        <Chip delay={64} size={34}>复制为可编辑</Chip>
        <Sparkle size={34} color={C.pink} style={{ marginTop: 6 }} />
      </div>
    </Backdrop>
  );
};

export const Skins: React.FC = () => (
  <>
    <Sequence from={0} durationInFrames={GRID_LEN}><Grid /></Sequence>
  </>
);
