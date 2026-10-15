import React from 'react';
import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';
import { C, FPS, SHADOW, fRound, fSans, fMono } from '../theme';
import { charTimes, visibleCount, Reveal } from '../markup';

export const useSpring = (delay = 0, cfg: { damping?: number; stiffness?: number; mass?: number } = {}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return spring({ frame: frame - delay, fps, config: { damping: 15, stiffness: 190, mass: 0.8, ...cfg } });
};

/** visible char count for text typed starting at `start` (local frame) */
export const useTyped = (text: string, start: number, reveal: Reveal = {}, speed = 1) => {
  const frame = useCurrentFrame();
  const times = React.useMemo(() => charTimes(text, reveal, speed), [text, reveal, speed]);
  return visibleCount(times, Math.max(0, ((frame - start) / FPS) * 1000));
};

export const Sparkle: React.FC<{ size?: number; color?: string; rot?: number; style?: React.CSSProperties }> = ({
  size = 24, color = C.sky, rot = 0, style,
}) => (
  <svg width={size} height={size} viewBox="0 0 100 100" style={{ transform: `rotate(${rot}deg)`, ...style }}>
    <polygon points="50,0 61,39 100,50 61,61 50,100 39,61 0,50 39,39" fill={color} />
  </svg>
);

const SP = Array.from({ length: 14 }, (_, i) => ({
  x: (i * 337) % 1920, y: (i * 211 + 60) % 1080, s: 14 + ((i * 7) % 22), p: i * 0.9,
  c: i % 5 === 0 ? C.pink : i % 2 ? C.sky : '#ffffff',
}));

export const Backdrop: React.FC<{ bg?: string; sparkles?: boolean; children?: React.ReactNode }> = ({
  bg, sparkles = true, children,
}) => {
  const f = useCurrentFrame();
  return (
    <AbsoluteFill
      style={{
        background: bg ?? `radial-gradient(1200px 700px at 30% 20%, #ffffff, ${C.ice} 70%, #d6e9f7)`,
        overflow: 'hidden',
      }}
    >
      {sparkles &&
        SP.map((s, i) => (
          <Sparkle
            key={i}
            size={s.s}
            color={s.c}
            rot={f * 0.6 + i * 20}
            style={{
              position: 'absolute',
              left: s.x,
              top: s.y + Math.sin(f / 30 + s.p) * 10,
              opacity: 0.35 + 0.35 * Math.sin(f / 18 + s.p),
            }}
          />
        ))}
      {children}
    </AbsoluteFill>
  );
};

export const Card: React.FC<{ style?: React.CSSProperties; children?: React.ReactNode }> = ({ style, children }) => (
  <div style={{ background: '#fff', borderRadius: 24, boxShadow: SHADOW, ...style }}>{children}</div>
);

export const Chip: React.FC<{ children: React.ReactNode; delay?: number; color?: string; dark?: boolean; size?: number }> = ({
  children, delay = 0, color, dark, size = 30,
}) => {
  const p = useSpring(delay);
  return (
    <div
      style={{
        display: 'inline-block',
        padding: `${size * 0.28}px ${size * 0.7}px`,
        borderRadius: 999,
        background: dark ? '#ffffff14' : color ?? `linear-gradient(90deg, ${C.blue}, ${C.blueLight})`,
        color: '#fff',
        fontFamily: fSans,
        fontWeight: 700,
        fontSize: size,
        boxShadow: dark ? 'none' : `0 3px 8px ${C.blue}55`,
        transform: `scale(${p}) translateY(${(1 - p) * 20}px)`,
        opacity: p,
        whiteSpace: 'nowrap',
      }}
    >
      {children}
    </div>
  );
};

export const Title: React.FC<{ kicker?: string; title: string; sub?: string; dark?: boolean; delay?: number; size?: number }> = ({
  kicker, title, sub, dark, delay = 0, size = 92,
}) => {
  const p = useSpring(delay);
  const p2 = useSpring(delay + 6);
  return (
    <div style={{ fontFamily: fSans }}>
      {kicker && (
        <div
          style={{
            fontFamily: fMono, fontWeight: 700, fontSize: 28, letterSpacing: 4,
            color: dark ? C.sky : C.blue, opacity: p, transform: `translateX(${(1 - p) * -30}px)`,
          }}
        >
          ✦ {kicker}
        </div>
      )}
      <div
        style={{
          fontWeight: 900, fontSize: size, lineHeight: 1.1, marginTop: 8,
          color: dark ? '#fff' : C.navy, transform: `translateX(${(1 - p) * -60}px)`, opacity: p,
        }}
      >
        {title}
      </div>
      {sub && (
        <div
          style={{
            fontFamily: fSans, fontWeight: 500, fontSize: 38, marginTop: 14,
            color: dark ? '#b9c6dd' : C.muted, opacity: p2, transform: `translateY(${(1 - p2) * 20}px)`,
          }}
        >
          {sub}
        </div>
      )}
    </div>
  );
};

export const KeyCap: React.FC<{ label: string; pressed: boolean; hint?: string; delay?: number }> = ({
  label, pressed, hint, delay = 0,
}) => {
  const p = useSpring(delay);
  const down = pressed ? 10 : 0;
  return (
    <div style={{ textAlign: 'center', transform: `scale(${p})`, opacity: p }}>
      <div
        style={{
          minWidth: 200, height: 200, padding: '0 40px', boxSizing: 'border-box', borderRadius: 36, position: 'relative',
          background: pressed ? `linear-gradient(180deg, ${C.blue}, ${C.blueLight})` : '#fff',
          color: pressed ? '#fff' : C.navy,
          fontFamily: fRound, fontWeight: 900, fontSize: 84,
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          transform: `translateY(${down}px)`,
          boxShadow: `0 ${16 - down}px 0 ${pressed ? '#0a5aa0' : C.line}, 0 ${28 - down}px 36px #1b2f4b33`,
          transition: 'none',
        }}
      >
        {label}
      </div>
      {hint && (
        <div style={{ fontFamily: fSans, fontWeight: 700, fontSize: 34, color: C.navy, marginTop: 40 }}>{hint}</div>
      )}
    </div>
  );
};
