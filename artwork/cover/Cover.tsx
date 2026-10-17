import React from 'react';
import { AbsoluteFill, Img, staticFile, useVideoConfig } from 'remotion';
import { loadFont } from '@remotion/google-fonts/NotoSansSC';

const fSans = loadFont('normal', { weights: ['700', '900'], ignoreTooManyRequestsWarning: true }).fontFamily;
const C = { navy: '#1b2f4b', pink: '#e86a92', blueLight: '#3aa6f0' };
const BOX_BACKGROUND = 'linear-gradient(160deg, #fffffff7 0%, #eef6fdf2 100%)';

const FEATURES = ['透明捕捉', '极低占用', '文字演出', '24 款皮肤'];

const Star: React.FC<{ x: number; y: number; size?: number; pink?: boolean }> = ({ x, y, size = 42, pink }) => (
  <svg width={size} height={size} viewBox="0 0 100 100" style={{ position: 'absolute', left: x, top: y }}>
    <path d="M50 0 Q56 43 100 50 Q56 57 50 100 Q44 57 0 50 Q44 43 50 0" fill={pink ? '#ef9ab8' : '#61b7f1'} />
  </svg>
);

const Background: React.FC = () => (
  <AbsoluteFill style={{ background: 'linear-gradient(180deg, #ffffff08, #529fdf18), repeating-linear-gradient(120deg, #ffffff 0px, #ffffff 76px, #8dc5ef 76px, #b3dcfa 152px)' }}>
    <AbsoluteFill style={{ background: 'radial-gradient(ellipse at 30% 20%, #ffffff38, transparent 68%)' }} />
    <svg width="100%" height="100%" viewBox="0 0 1920 1080" preserveAspectRatio="xMidYMid slice" aria-hidden style={{ position: 'absolute', inset: 0 }}>
      <defs>
        <pattern id="cover-dots" width="22" height="22" patternUnits="userSpaceOnUse"><circle cx="4" cy="4" r="3" fill="#2884ca" /></pattern>
      </defs>
      <path d="M0 286 H235 V710 H0 Z" fill="url(#cover-dots)" opacity="0.18" />
      <path d="M1570 0 H1920 V228 H1570 Z" fill="url(#cover-dots)" opacity="0.18" />
      <g fill="none" stroke="#ffffff" strokeWidth="8" opacity="0.65">
        <ellipse cx="1520" cy="346" rx="504" ry="322" transform="rotate(-20 1520 346)" />
        <ellipse cx="1520" cy="346" rx="540" ry="356" transform="rotate(-20 1520 346)" strokeWidth="3" />
      </g>
      <g transform="translate(-88 858) rotate(-14)" fill="#ffffffa6" stroke="#4b9cd8" strokeWidth="5" opacity="0.6">
        <rect width="602" height="264" rx="34" />
        {Array.from({ length: 16 }, (_, i) => <rect key={i} x={24 + i % 8 * 70} y={24 + Math.floor(i / 8) * 70} width="54" height="52" rx="9" />)}
        <rect x="24" y="168" width="96" height="58" rx="10" />
        <rect x="138" y="168" width="310" height="58" rx="10" />
        <rect x="466" y="168" width="110" height="58" rx="10" />
      </g>
      <g transform="translate(92 112) rotate(-12 102 76)" stroke="#d76588" strokeWidth="7" fill="#fff1f6bb" opacity="0.65">
        <path d="M24 0 H178 Q204 0 204 26 V104 Q204 130 178 130 H84 L38 164 V130 H24 Q0 130 0 104 V26 Q0 0 24 0Z" />
        <path d="M46 50 H152 M46 80 H119" fill="none" strokeLinecap="round" />
      </g>
      <g transform="translate(860 28) rotate(14 56 56)" fill="#ffe2a3cc" stroke="#ae833c" strokeWidth="6" opacity="0.7">
        <rect width="112" height="112" rx="24" />
        <path d="M80 32 V66 H35 M51 50 L35 66 L51 82" fill="none" strokeLinecap="round" strokeLinejoin="round" />
      </g>
      <path d="M90 562 L90 734 L136 688 L168 752 L201 734 L167 670 H230 Z" fill="#ffffffc9" stroke="#4c99d1" strokeWidth="7" strokeLinejoin="round" opacity="0.65" transform="rotate(-10 150 650)" />
      {[[196, 42, 26], [756, 346, 28], [211, 748, 22], [1716, 910, 36], [830, 580, 19]].map(([x, y, r], i) => <path key={i} d={`M${x} ${y - r} Q${x + 4} ${y - 4} ${x + r} ${y} Q${x + 4} ${y + 4} ${x} ${y + r} Q${x - 4} ${y + 4} ${x - r} ${y} Q${x - 4} ${y - 4} ${x} ${y - r}`} fill={i % 2 ? '#f0a944' : '#ffffff'} opacity="0.8" />)}
    </svg>
  </AbsoluteFill>
);

const Brand: React.FC<{ compact?: boolean }> = ({ compact }) => (
  <div style={{ width: '100%', textAlign: 'center', fontFamily: fSans, fontSize: compact ? 202 : 290, fontWeight: 900, lineHeight: 1.12, letterSpacing: compact ? -9 : -14, color: '#192b49', whiteSpace: 'nowrap', WebkitTextStroke: compact ? '22px white' : '30px white', paintOrder: 'stroke fill', textShadow: '0 10px 20px #192b4933' }}>NanaType</div>
);

const FeatureCapsules: React.FC<{ portrait?: boolean }> = ({ portrait }) => (
  <div style={{ display: 'flex', flexDirection: 'column', gap: portrait ? 24 : 28 }}>
    {['表情与口型同步', '直播间实时互动'].map((title, i) => (
      <div key={title} style={{ height: portrait ? 106 : 116, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: '0 26px', borderRadius: 999, border: '5px solid white', background: i === 0 ? 'linear-gradient(115deg, #ffb4c5, #f481a2)' : 'linear-gradient(115deg, #ffe8a0, #f9c75c)', boxShadow: i === 0 ? '0 9px 0 #be5579, 0 12px 28px #63304224' : '0 9px 0 #b98b32, 0 12px 28px #694f1924', color: '#192b49', whiteSpace: 'nowrap' }}>
        <span style={{ fontSize: portrait ? 52 : 70, fontWeight: 900, letterSpacing: -2 }}>{title}</span>
      </div>
    ))}
  </div>
);

/** Freeze the text effects at a clear, deterministic point for static exports. */
const PerformedText: React.FC<{ text: string; rainbow?: boolean }> = ({ text, rainbow }) => {
  const phase = 24 * Math.PI * 2 * 1.1;
  const colors = ['#c33e70', '#b46b20', '#8f49bd', '#31558f'];
  return <span style={{ display: 'inline-flex', fontWeight: 900 }}>{Array.from(text).map((char, i) => (
    <span key={i} style={{ display: 'inline-block', transform: `translateY(${Math.sin(phase + i * 0.95) * (rainbow ? 9 : 4)}px)`, color: rainbow ? colors[i % colors.length] : i % 2 ? '#5f467e' : '#192b49', textShadow: '0 2px 0 #fff' }}>{char}</span>
  ))}</span>;
};

const Character: React.FC<{ style?: React.CSSProperties }> = ({ style }) => (
  <Img src={staticFile('character.png')} style={{ position: 'absolute', objectFit: 'contain', filter: 'drop-shadow(12px 0 0 #ffffff) drop-shadow(-12px 0 0 #ffffff) drop-shadow(0 -12px 0 #ffffff) drop-shadow(0 12px 0 #ffffff) drop-shadow(0 18px 24px #286c9e38)', ...style }} />
);

/** The product positioning itself is the typed line; keep it large in a 4:3 crop. */
const InputBox: React.FC<{ portrait?: boolean }> = ({ portrait }) => (
  <div style={{
    position: 'relative', width: '100%', height: '100%', boxSizing: 'border-box',
    display: 'flex', flexDirection: 'column', justifyContent: 'center', alignItems: 'center', textAlign: 'center',
    borderRadius: 40, border: '5px solid white', background: BOX_BACKGROUND,
    boxShadow: '0 0 0 3px #314662, 0 20px 50px #192b4933, inset 0 2px 0 white', padding: portrait ? '28px 32px' : '24px 44px',
  }}>
    <div style={{ position: 'absolute', left: 24, top: 62, width: 6, height: 140, borderRadius: 4, background: 'linear-gradient(#d94d78, #f7a8bc)' }} />
    <div style={{ position: 'absolute', right: 10, top: -7, width: portrait ? 76 : 92, height: portrait ? 46 : 56, borderRadius: 46, transform: 'rotate(-8deg)', background: '#7ebceba6' }} />
    <div style={{ position: 'absolute', right: portrait ? 114 : 136, top: 26, width: 10, height: 10, borderRadius: 5, background: '#e99ab38c' }} />
    <div style={{ fontSize: portrait ? 82 : 126, fontWeight: 900, lineHeight: 1.25, letterSpacing: -2, whiteSpace: 'nowrap' }}>
      开源免费<span style={{ color: '#d04671' }}>打字机</span>
    </div>
    <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', marginTop: 18, fontSize: portrait ? 41 : 60, fontWeight: 700, lineHeight: 1.35, whiteSpace: 'nowrap', letterSpacing: -1 }}>
      {FEATURES.map((feature, i) => (
        <React.Fragment key={feature}>
          {i > 0 && <span style={{ margin: portrait ? '0 7px' : '0 16px', color: '#7d8b9f' }}>·</span>}
          {i === 2 ? <PerformedText text={feature} rainbow /> : i === 3 ? <PerformedText text={feature} /> : <span style={{ color: '#192b49' }}>{feature}</span>}
        </React.Fragment>
      ))}
      <span style={{ display: 'inline-block', width: portrait ? 5 : 7, height: portrait ? 41 : 60, background: C.pink, marginLeft: 12, borderRadius: 4 }} />
    </div>
    <div style={{ position: 'absolute', bottom: 16, left: '50%', transform: 'translateX(-50%)', width: 0, height: 0, borderLeft: '12px solid transparent', borderRight: '12px solid transparent', borderTop: `10px solid ${C.blueLight}` }} />
  </div>
);

/** Dense landscape composition; the central 4:3 crop retains the full brand and copy. */
export const Cover: React.FC<{ cropToFourThree?: boolean }> = ({ cropToFourThree = false }) => {
  const { width, height } = useVideoConfig();
  const scale = cropToFourThree ? height / 1080 : Math.min(width / 1920, height / 1080);
  return (
    <AbsoluteFill style={{ background: '#e4f3ff', overflow: 'hidden' }}>
      <Background />
      <div style={{ position: 'absolute', left: (width - 1920 * scale) / 2, top: (height - 1080 * scale) / 2, width: 1920, height: 1080, transform: `scale(${scale})`, transformOrigin: 'top left', fontFamily: fSans, fontWeight: 700, color: C.navy }}>
        <Character style={{ left: 400, top: -180, width: 1680, height: 2100 }} />
        <Star x={1770} y={384} size={64} />
        <div style={{ position: 'absolute', left: 264, top: 66, width: 1392 }}><Brand /></div>
        <div style={{ position: 'absolute', left: 286, top: 418, width: 682 }}><FeatureCapsules /></div>
        <div style={{ position: 'absolute', left: 264, top: 710, width: 1392, height: 320 }}><InputBox /></div>
      </div>
    </AbsoluteFill>
  );
};

export const DouyinCover: React.FC = () => (
  <AbsoluteFill style={{ overflow: 'hidden', fontFamily: fSans, color: C.navy }}>
    <Background />
    <Character style={{ left: -30, top: -5, width: 1440, height: 1850 }} />
    <div style={{ position: 'absolute', left: 64, top: 80, width: 952 }}><Brand compact /></div>
    <div style={{ position: 'absolute', left: 64, top: 352, width: 532 }}><FeatureCapsules portrait /></div>
    <Star x={900} y={350} size={54} />
    <div style={{ position: 'absolute', left: 64, top: 1060, width: 952, height: 312 }}><InputBox portrait /></div>
  </AbsoluteFill>
);
