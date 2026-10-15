import React from 'react';
import { interpolate, Sequence, useCurrentFrame } from 'remotion';
import { Backdrop, Card, Chip, KeyCap, Title, useSpring, useTyped } from '../components/ui';
import { ScaledSkin } from '../components/ScaledSkin';
import { parseMarkup, plain } from '../markup';
import { C, fMono, fSans } from '../theme';
import { BeatAutoHide, BeatMirror, BeatSticker } from './FeaturesExtra';

export const BEAT = 75;
export const AUTO_LEN = 90;
export const FEAT_LEN = BEAT * 5 + AUTO_LEN;

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const;

const Left: React.FC<{ kicker: string; title: string; sub: string }> = ({ kicker, title, sub }) => (
  <div style={{ position: 'absolute', left: 100, top: 120, width: 760 }}>
    <Title kicker={kicker} title={title} sub={sub} size={96} />
  </div>
);

/* A — rich text */
const CODE = ['[color=#e86a92]红色[/color]', '[wave]波浪[/wave]', '[ruby=かんじ]漢字[/ruby]', '[shake]抖动[/shake]'];
const RESULT = parseMarkup('[color=#e86a92]红色[/color]、[wave]波浪[/wave]、[ruby=かんじ]漢字[/ruby]、[shake]抖动[/shake]');
const BeatRich: React.FC = () => {
  const n = useTyped(plain(RESULT), 30, { base_ms: 50, cjk_factor: 1.2 });
  const card = useSpring(24);
  return (
    <Backdrop>
      <Left kicker="RICH TEXT" title="富文本标记" sub="颜色 · 注音 · 描边 · 逐字特效" />
      <Card style={{ position: 'absolute', left: 980, top: 130, width: 840, padding: '34px 44px' }}>
        {CODE.map((c, i) => <CodeLine key={i} text={c} delay={4 + i * 6} />)}
      </Card>
      <div style={{ position: 'absolute', left: 100, top: 640, opacity: card, transform: `translateY(${(1 - card) * 60}px)` }}>
        <ScaledSkin id="halo" width={1500} regionH={130} name="奈奈" spans={RESULT} visible={n} showContinue={false} />
      </div>
    </Backdrop>
  );
};
const CodeLine: React.FC<{ text: string; delay: number }> = ({ text, delay }) => {
  const p = useSpring(delay);
  return (
    <div style={{ fontFamily: fMono, fontWeight: 700, fontSize: 38, color: C.blue, lineHeight: 1.7, opacity: p, transform: `translateX(${(1 - p) * 40}px)` }}>
      {text}
    </div>
  );
};

/* B — rhythm markers */
const SEGS: { t: string; m?: string }[] = [
  { t: '等一下', m: 'pause' }, { t: '看这里', m: 'page' }, { t: '叮～', m: 'se=ding' }, { t: '开心！', m: 'expr=smile' },
];
const BeatRhythm: React.FC = () => {
  const f = useCurrentFrame();
  return (
    <Backdrop>
      <Left kicker="RHYTHM" title="节奏由你掌控" sub="停顿 · 分页 · 音效 · 表情" />
      <Card style={{ position: 'absolute', left: 100, top: 560, width: 1720, height: 330 }}>
        <div style={{ display: 'flex', alignItems: 'flex-end', justifyContent: 'center', gap: 0, height: '100%', paddingBottom: 90 }}>
          {SEGS.map((s, i) => {
            const d = 8 + i * 12;
            const p = interpolate(f, [d, d + 8], [0, 1], clamp);
            return (
              <div key={i} style={{ display: 'flex', alignItems: 'flex-end' }}>
                <span style={{ fontFamily: fSans, fontWeight: 900, fontSize: 92, color: C.navy, opacity: interpolate(f, [d - 6, d], [0, 1], clamp) }}>{s.t}</span>
                <div style={{ position: 'relative', width: 8, height: 130, margin: '0 36px', borderRadius: 4, background: i === 3 ? C.pink : C.blue, transform: `scaleY(${p})`, transformOrigin: 'bottom' }}>
                  <div style={{ position: 'absolute', left: '50%', top: -64, transform: `translateX(-50%) scale(${p})` }}>
                    <span
                      style={{
                        display: 'inline-block', fontFamily: fMono, fontWeight: 700, fontSize: 28, color: '#fff', whiteSpace: 'nowrap',
                        background: i === 3 ? C.pink : `linear-gradient(90deg, ${C.blue}, ${C.blueLight})`, borderRadius: 999, padding: '4px 16px',
                      }}
                    >
                      [{s.m}]
                    </span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </Card>
    </Backdrop>
  );
};

/* C — hotkeys */
const BeatKeys: React.FC = () => {
  const f = useCurrentFrame();
  const win = useSpring(18, { damping: 12 });
  const f8 = f >= 12 && f < 26;
  const send = f >= 34 && f < 46;
  const f9 = f >= 54 && f < 68;
  const adv = interpolate(f, [36, 42], [0, 1], clamp);
  return (
    <Backdrop>
      <Left kicker="HOTKEYS" title="全局热键" sub="游戏里也能随时开口" />
      <div style={{ position: 'absolute', left: 980, top: 130, width: 840, height: 420, borderRadius: 28, overflow: 'hidden', background: 'linear-gradient(135deg,#2b3a67,#6b4f9e 60%,#e98aa8)', boxShadow: '0 18px 48px #1b2f4b44' }}>
        <div style={{ position: 'absolute', left: 20, top: 14, fontFamily: fSans, color: '#ffffffaa', fontWeight: 700, fontSize: 24 }}>▶ 全屏游戏中…</div>
        <div style={{ position: 'absolute', left: 120, top: 100 + (1 - win) * 80, width: 600, opacity: win, transform: `scale(${0.8 + win * 0.2})` }}>
          <div style={{ background: '#fff', borderRadius: 18, padding: '18px 24px', fontFamily: fSans, fontSize: 32, color: C.navy, fontWeight: 500, boxShadow: '0 10px 30px #00000055' }}>
            {adv > 0.5 ? '下一句台词～' : '输入台词，回车发送…'}
            <span style={{ display: 'inline-block', width: 4, height: 34, background: C.pink, marginLeft: 4, verticalAlign: 'middle', opacity: Math.floor(f / 8) % 2 ? 0.2 : 1 }} />
          </div>
        </div>
      </div>
      <div style={{ position: 'absolute', left: 980, top: 590, display: 'flex', gap: 56 }}>
        <KeyCap label="F8" pressed={f8} hint="唤起输入" delay={4} />
        <KeyCap label="Enter" pressed={send} hint="发送台词" delay={8} />
        <KeyCap label="F9" pressed={f9} hint="继续 / 跳过" delay={12} />
      </div>
      <div style={{ position: 'absolute', left: 980, top: 910, fontFamily: fSans, fontWeight: 500, fontSize: 28, color: C.muted }}>
        Shift+Enter 换行 · 可改为 Ctrl+Enter 发送 · F8 / F9 可自定义
      </div>
    </Backdrop>
  );
};

export const Features: React.FC = () => (
  <>
    <Sequence from={0} durationInFrames={BEAT}><BeatRich /></Sequence>
    <Sequence from={BEAT} durationInFrames={BEAT}><BeatRhythm /></Sequence>
    <Sequence from={BEAT * 2} durationInFrames={BEAT}><BeatKeys /></Sequence>
    <Sequence from={BEAT * 3} durationInFrames={BEAT}><BeatMirror /></Sequence>
    <Sequence from={BEAT * 4} durationInFrames={BEAT}><BeatSticker /></Sequence>
    <Sequence from={BEAT * 5} durationInFrames={AUTO_LEN}><BeatAutoHide /></Sequence>
  </>
);

void Chip;
