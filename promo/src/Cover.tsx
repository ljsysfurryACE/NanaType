import React from 'react';
import { AbsoluteFill, Img, staticFile, useVideoConfig } from 'remotion';
import skins from './skins.generated.json';
import { C, fRound, fSans } from './theme';

const halo = skins.halo.dialogue;
const gradient = (value: { angle?: number; stops: (string | number)[][] }) =>
  `linear-gradient(${value.angle ?? 180}deg, ${value.stops.map(([at, color]) => `${color} ${Number(at) * 100}%`).join(', ')})`;

/** A close-up of the left of Halo's dialogue box, with thumbnail-sized type. */
export const Cover: React.FC<{ cropToFourThree?: boolean }> = ({ cropToFourThree = false }) => {
  const { width, height } = useVideoConfig();
  const scale = cropToFourThree ? height / 1080 : Math.min(width / 1920, height / 1080);

  return (
    <AbsoluteFill style={{ background: C.ice, overflow: 'hidden' }}>
      <div
        style={{
          position: 'absolute', width: 1920, height: 1080,
          left: (width - 1920 * scale) / 2, top: (height - 1080 * scale) / 2,
          transform: `scale(${scale})`, transformOrigin: 'top left',
          fontFamily: fSans, fontWeight: 900, color: C.navy,
        }}
      >
        {/* The right edge stays out of frame: this is the box's left portion. */}
        <div
          style={{
            position: 'absolute', left: 176, top: 222, width: 2600, height: 818,
            borderRadius: halo.box.radius * 3.2,
            border: `3px solid ${halo.box.border_color}`,
            background: gradient(halo.box.gradient),
            boxShadow: '0 26px 72px #1b2f4b25, inset 0 3px 0 #ffffff',
          }}
        />

        {/* Halo's blue nameplate, now carrying the product's actual logo. */}
        <div
          style={{
            position: 'absolute', left: 280, top: 68, width: 1360, height: 246,
            display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 36,
            borderRadius: halo.nameplate.radius,
            background: gradient(halo.nameplate.gradient),
            boxShadow: '0 16px 36px #0876d040, inset 0 2px 0 #ffffff50',
          }}
        >
          <Img src={staticFile('nanatype-256.png')} style={{ width: 194, height: 194 }} />
          <span style={{ fontFamily: fRound, fontSize: 206, lineHeight: 1, letterSpacing: -6, color: '#fff', transform: 'translateY(-14px)' }}>
            NanaType
          </span>
        </div>

        <div
          style={{
            position: 'absolute', left: 228, top: 368, width: 14, height: 604,
            borderRadius: 7, background: `linear-gradient(${C.blue}, ${C.sky})`,
          }}
        />

        <div style={{ position: 'absolute', left: 280, width: 1360, top: 340, textAlign: 'center', fontSize: 160, lineHeight: 1.3, letterSpacing: -4 }}>
          虚拟主播打字机
        </div>

        {/* All essential content stays within x=280…1640. Even a centered
            4:3 crop of the 16:9 version (x=240…1680) keeps 40px clearance. */}
        <div
          style={{
            position: 'absolute', left: 280, top: 578, width: 1360,
            display: 'grid', gridTemplateColumns: '680px 680px', rowGap: 28,
            fontSize: 148, lineHeight: 1.3, letterSpacing: -4, whiteSpace: 'nowrap',
            textAlign: 'center', alignItems: 'center',
          }}
        >
          <div style={{ color: '#0767b6' }}>VTS 控制</div>
          <div>弹幕互动</div>
          <div style={{ color: '#cf3f79', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 16 }}>
            文字演出
            <span style={{ width: 12, height: 144, borderRadius: 6, background: C.pink }} />
          </div>
          <div style={{ color: '#0767b6' }}>18 款皮肤</div>
        </div>
      </div>
    </AbsoluteFill>
  );
};

/** A dedicated 3:4 layout: keep the title and feature labels large on phones. */
export const DouyinCover: React.FC = () => (
  <AbsoluteFill
    style={{
      background: `radial-gradient(ellipse at 30% 15%, #ffffff, ${C.ice} 70%, #d6e9f7)`,
      overflow: 'hidden', fontFamily: fSans, fontWeight: 900, color: C.navy,
    }}
  >
    <div
      style={{
        position: 'absolute', left: 48, top: 210, width: 984, height: 1134,
        boxSizing: 'border-box', borderRadius: halo.box.radius * 3.2,
        border: `3px solid ${halo.box.border_color}`, background: gradient(halo.box.gradient),
        boxShadow: '0 26px 72px #1b2f4b25, inset 0 3px 0 #ffffff',
      }}
    />

    <div
      style={{
        position: 'absolute', left: 94, top: 104, width: 892, height: 176,
        display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 24,
        borderRadius: halo.nameplate.radius, background: gradient(halo.nameplate.gradient),
        boxShadow: '0 16px 36px #0876d040, inset 0 2px 0 #ffffff50',
      }}
    >
      <Img src={staticFile('nanatype-256.png')} style={{ width: 132, height: 132 }} />
      <span style={{ fontFamily: fRound, fontSize: 132, lineHeight: 1, letterSpacing: -4, color: '#fff', transform: 'translateY(-9px)' }}>
        NanaType
      </span>
    </div>

    <div
      style={{
        position: 'absolute', left: 88, top: 364, width: 10, height: 824,
        borderRadius: 5, background: `linear-gradient(${C.blue}, ${C.sky})`,
      }}
    />

    <div style={{ position: 'absolute', left: 120, top: 354, width: 840, textAlign: 'center', fontSize: 172, lineHeight: 1.25, letterSpacing: -6 }}>
      虚拟主播
    </div>
    <div
      style={{
        position: 'absolute', left: 120, top: 562, width: 840,
        display: 'flex', justifyContent: 'center', alignItems: 'center', gap: 18,
        fontSize: 224, lineHeight: 1.2, letterSpacing: -8, color: '#0767b6',
      }}
    >
      打字机
      <span style={{ width: 12, height: 204, borderRadius: 6, background: C.pink, marginTop: 20 }} />
    </div>
    <div style={{ position: 'absolute', left: 120, top: 858, width: 840, textAlign: 'center', fontSize: 40, fontWeight: 700, color: C.muted, letterSpacing: 2 }}>
      你打的每一句，都是台词
    </div>

    <div
      style={{
        position: 'absolute', left: 120, top: 990, width: 840,
        display: 'grid', gridTemplateColumns: '1fr 1fr', rowGap: 34,
        fontSize: 80, lineHeight: 1.3, letterSpacing: -3, whiteSpace: 'nowrap', textAlign: 'center',
      }}
    >
      <div style={{ color: '#0767b6' }}>VTS 控制</div>
      <div>弹幕互动</div>
      <div style={{ color: '#cf3f79' }}>文字演出</div>
      <div style={{ color: '#0767b6' }}>18 款皮肤</div>
    </div>

    <div
      style={{
        position: 'absolute', left: 520, top: 1258, width: 0, height: 0,
        borderLeft: '20px solid transparent', borderRight: '20px solid transparent', borderTop: `16px solid ${C.blueLight}`,
      }}
    />
  </AbsoluteFill>
);
