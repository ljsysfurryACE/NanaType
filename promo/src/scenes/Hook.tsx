import React from 'react';
import { Img, interpolate, staticFile, useCurrentFrame } from 'remotion';
import { Backdrop, Sparkle, useSpring, useTyped, Chip } from '../components/ui';
import { C, fRound, fSans } from '../theme';

const LINE = '你打的每一句，都是台词';

export const Hook: React.FC = () => {
  const f = useCurrentFrame();
  const n = useTyped(LINE, 4, { base_ms: 50, cjk_factor: 1.4, punct: { comma_ms: 60 } });
  const out = interpolate(f, [56, 66], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
  const icon = useSpring(58, { damping: 11, stiffness: 150 });
  const word = useSpring(68);
  const burst = Array.from({ length: 8 }, (_, i) => i);
  const p = interpolate(f, [58, 84], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
  return (
    <Backdrop>
      <div
        style={{
          position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center',
          opacity: out, transform: `translateY(${(1 - out) * -40}px)`,
        }}
      >
        <div style={{ fontFamily: fSans, fontWeight: 900, fontSize: 96, color: C.navy, display: 'flex', alignItems: 'center' }}>
          {Array.from(LINE).slice(0, n).join('')}
          <span
            style={{
              display: 'inline-block', width: 10, height: 104, marginLeft: 10, borderRadius: 5, background: C.pink,
              opacity: Math.floor(f / 8) % 2 ? 0.15 : 1,
            }}
          />
        </div>
      </div>

      <div style={{ position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
        {burst.map((i) => {
          const a = (i / burst.length) * Math.PI * 2 + 0.3;
          const r = 160 + p * 300;
          return (
            <Sparkle
              key={i}
              size={30 + (i % 3) * 14}
              color={i % 3 === 0 ? C.pink : C.sky}
              style={{
                position: 'absolute', left: 960 + Math.cos(a) * r - 20, top: 460 + Math.sin(a) * r - 20,
                opacity: p === 0 ? 0 : 1 - p,
              }}
              rot={p * 90}
            />
          );
        })}
        <Img
          src={staticFile('nanatype-256.png')}
          style={{
            width: 280, height: 280, borderRadius: 64,
            transform: `scale(${icon}) rotate(${(1 - icon) * -25}deg)`, opacity: icon,
            boxShadow: '0 24px 56px #0876d055',
          }}
        />
        <div
          style={{
            fontFamily: fRound, fontWeight: 900, fontSize: 150, marginTop: 24, letterSpacing: -2,
            background: `linear-gradient(90deg, ${C.blue}, ${C.blueLight})`, WebkitBackgroundClip: 'text', color: 'transparent',
            transform: `translateY(${(1 - word) * 40}px)`, opacity: word,
          }}
        >
          NanaType
        </div>
        <Chip delay={76} size={38}>虚拟主播打字机</Chip>
      </div>
    </Backdrop>
  );
};
