import React from 'react';
import { AbsoluteFill, Audio, Composition, interpolate, Sequence, staticFile, Still, useCurrentFrame } from 'remotion';
import { Hook } from './scenes/Hook';
import { Typewriter } from './scenes/Typewriter';
import { Features, BEAT, FEAT_LEN } from './scenes/Features';
import { Skins, GRID_LEN } from './scenes/Skins';
import { Output, Outro } from './scenes/OutputOutro';
import { Blivechat, VtsEvents } from './scenes/Link';
import { SkinSheet } from './SkinSheet';
import { Cover, DouyinCover } from './Cover';
import { C, FPS, H, W } from './theme';

// scene starts (frames) — keep in sync with scripts/gen-audio.mjs
const S0 = 270 + FEAT_LEN;
const S = {
  hook: 0,
  type: 90,
  feat: 270,
  skins: S0,
  bili: S0 + GRID_LEN,
  vts: S0 + GRID_LEN + 150,
  out: S0 + GRID_LEN + 285,
  outro: S0 + GRID_LEN + 405,
  end: S0 + GRID_LEN + 515,
};
const CUTS = [
  S.type, S.feat, S.feat + BEAT, S.feat + BEAT * 2, S.feat + BEAT * 3, S.feat + BEAT * 4, S.feat + BEAT * 5,
  S.skins, S.bili, S.vts, S.out, S.outro,
];

const Wipe: React.FC<{ at: number }> = ({ at }) => {
  const f = useCurrentFrame();
  const x = interpolate(f, [at - 6, at + 6], [-110, 110], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
  if (f < at - 7 || f > at + 7) return null;
  return (
    <AbsoluteFill
      style={{
        transform: `translateX(${x}%) skewX(-12deg)`,
        background: `linear-gradient(90deg, ${C.blue}, ${C.blueLight})`,
        boxShadow: '0 0 80px #0876d0aa',
      }}
    />
  );
};

const Promo: React.FC = () => (
  <AbsoluteFill style={{ background: C.ice }}>
    <Sequence from={S.hook} durationInFrames={S.type - S.hook}><Hook /></Sequence>
    <Sequence from={S.type} durationInFrames={S.feat - S.type}><Typewriter /></Sequence>
    <Sequence from={S.feat} durationInFrames={S.skins - S.feat}><Features /></Sequence>
    <Sequence from={S.skins} durationInFrames={S.bili - S.skins}><Skins /></Sequence>
    <Sequence from={S.bili} durationInFrames={S.vts - S.bili}><Blivechat /></Sequence>
    <Sequence from={S.vts} durationInFrames={S.out - S.vts}><VtsEvents /></Sequence>
    <Sequence from={S.out} durationInFrames={S.outro - S.out}><Output /></Sequence>
    <Sequence from={S.outro} durationInFrames={S.end - S.outro}><Outro /></Sequence>
    {CUTS.map((c) => <Wipe key={c} at={c} />)}
    <Audio src={staticFile('audio/bgm.wav')} volume={0.5} />
    <Audio src={staticFile('audio/sfx.wav')} volume={1} />
  </AbsoluteFill>
);

export const Root: React.FC = () => (
  <>
    <Composition id="Promo" component={Promo} width={W} height={H} fps={FPS} durationInFrames={S.end} />
    <Composition id="ChatParity" component={Blivechat} width={W} height={H} fps={FPS} durationInFrames={150} />
    <Composition id="SkinSheet" component={SkinSheet} width={1920} height={1080} fps={30} durationInFrames={30} />
    <Still id="BilibiliCover" component={Cover} width={1920} height={1200} />
    <Still id="BilibiliCover16x9" component={Cover} width={1920} height={1080} />
    <Still id="BilibiliCover4x3" component={Cover} defaultProps={{ cropToFourThree: true }} width={1440} height={1080} />
    <Still id="DouyinCover" component={DouyinCover} width={1080} height={1440} />
  </>
);
