import React from 'react';
import { interpolate, useCurrentFrame } from 'remotion';
import { Backdrop, Card, Chip, Title, useSpring, useTyped } from '../components/ui';
import { ScaledSkin } from '../components/ScaledSkin';
import { parseMarkup, plain } from '../markup';
import { C, fMono, fSans } from '../theme';

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const;

const Left: React.FC<{ kicker: string; title: string; sub: string }> = ({ kicker, title, sub }) => (
  <div style={{ position: 'absolute', left: 100, top: 120, width: 760 }}>
    <Title kicker={kicker} title={title} sub={sub} size={96} />
  </div>
);

const Cursor: React.FC<{ f: number }> = ({ f }) => (
  <span
    style={{
      display: 'inline-block', width: 5, height: '1.1em', marginLeft: 4, verticalAlign: 'text-bottom', borderRadius: 3,
      background: C.pink, opacity: Math.floor(f / 7) % 2 ? 0.15 : 1,
    }}
  />
);

/* ---------- live mirror ---------- */
// (frame, op) — a char appends, null deletes the last char
const OPS: [number, string | null][] = [
  ...Array.from('今天来聊点什么呢？').map((c, i) => [12 + i * 3, c] as [number, string]),
  [46, null], [49, null], [52, null], [55, null],
  ...Array.from('游戏吧！').map((c, i) => [60 + i * 3, c] as [number, string]),
];
const textAt = (f: number) => {
  let t = '';
  for (const [fr, op] of OPS) if (f >= fr) t = op === null ? t.slice(0, -1) : t + op;
  return t;
};

export const BeatMirror: React.FC = () => {
  const f = useCurrentFrame();
  const t = textAt(f);
  const spans = [{ text: t }];
  const a = useSpring(2);
  const b = useSpring(8);
  return (
    <Backdrop>
      <Left kicker="LIVE MIRROR" title="实时镜像" sub="你打一个字，展示框跟一个字" />
      <div style={{ position: 'absolute', left: 980, top: 150, width: 840 }}>
        <div style={{ opacity: a, transform: `translateY(${(1 - a) * 40}px)` }}>
          <Chip size={26} color={C.navy}>输入框</Chip>
          <Card style={{ marginTop: 14, padding: '26px 34px', fontFamily: fSans, fontWeight: 500, fontSize: 52, color: C.navy, minHeight: 100 }}>
            {t}
            <Cursor f={f} />
          </Card>
        </div>
        <div style={{ textAlign: 'center', fontFamily: fMono, fontWeight: 700, fontSize: 40, color: C.blue, margin: '26px 0 10px' }}>▼ 同步</div>
        <div style={{ opacity: b, transform: `translateY(${(1 - b) * 40}px)` }}>
          <Chip size={26}>展示框</Chip>
          <div style={{ marginTop: 14 }}>
            <ScaledSkin id="halo" width={840} regionH={140} name="奈奈" spans={spans} visible={t.length} showContinue={false} />
          </div>
        </div>
      </div>
      <div style={{ position: 'absolute', left: 100, top: 560, display: 'flex', gap: 16, flexWrap: 'wrap', width: 760 }}>
        <Chip delay={20} size={30}>退格同步回退</Chip>
        <Chip delay={28} size={30}>字间隔可调</Chip>
        <Chip delay={36} size={30} color={C.pink}>● 镜像中</Chip>
      </div>
    </Backdrop>
  );
};

/* ---------- auto hide ---------- */
const LINE = parseMarkup('这句播完，就会自动淡出～');
const LINE_T = plain(LINE);
const OPTIONS = ['保持显示', '渐隐消失', '直接清除'];

export const BeatAutoHide: React.FC = () => {
  const f = useCurrentFrame();
  const n = useTyped(LINE_T, 6, { base_ms: 50, cjk_factor: 1.5 }, 1.4);
  const fade = interpolate(f, [60, 74], [1, 0], clamp);
  const count = interpolate(f, [34, 60], [100, 0], clamp);
  const seg = useSpring(2);
  // summon-mode input: appears, Enter at 50, slides away
  const inp = useSpring(10);
  const sent = interpolate(f, [58, 70], [0, 1], clamp);
  const press = f >= 52 && f < 60;
  const ghost = interpolate(f, [70, 78], [0, 1], clamp);
  return (
    <Backdrop>
      <Left kicker="AUTO HIDE" title="自动隐藏" sub="播完自动渐隐，发送后输入框自动收起" />

      {/* playback: after-line */}
      <div style={{ position: 'absolute', left: 980, top: 90, width: 840 }}>
        <div style={{ display: 'flex', gap: 10, opacity: seg }}>
          {OPTIONS.map((o, i) => (
            <span
              key={o}
              style={{
                fontFamily: fSans, fontWeight: 700, fontSize: 28, padding: '6px 22px', borderRadius: 999,
                background: i === 1 ? `linear-gradient(90deg, ${C.blue}, ${C.blueLight})` : '#fff',
                color: i === 1 ? '#fff' : C.muted, boxShadow: i === 1 ? `0 3px 8px ${C.blue}55` : '0 2px 6px #1b2f4b22',
              }}
            >
              {o}
            </span>
          ))}
          <span style={{ fontFamily: fMono, fontWeight: 700, fontSize: 28, color: C.pink, alignSelf: 'center', marginLeft: 10 }}>停留 2 秒</span>
        </div>
        <div style={{ marginTop: 26, opacity: fade, transform: `translateY(${(1 - fade) * -26}px)`, filter: `blur(${(1 - fade) * 6}px)` }}>
          <ScaledSkin id="halo" width={840} regionH={140} name="奈奈" spans={LINE} visible={n} showContinue={false} />
        </div>
        <div style={{ marginTop: 14, height: 8, borderRadius: 4, background: C.line, overflow: 'hidden', opacity: f >= 34 && f < 76 ? 1 : 0 }}>
          <div style={{ width: `${count}%`, height: '100%', background: `linear-gradient(90deg, ${C.blue}, ${C.pink})` }} />
        </div>
      </div>

      {/* summon mode: input auto-hides after sending */}
      <div style={{ position: 'absolute', left: 980, top: 600, width: 840 }}>
        <div style={{ fontFamily: fMono, fontWeight: 700, fontSize: 28, color: C.muted, marginBottom: 14 }}>✦ 唤出模式</div>
        <div
          style={{
            opacity: inp * (1 - sent), transform: `translateY(${sent * 60 + (1 - inp) * 40}px) scale(${1 - sent * 0.06})`,
            display: 'flex', alignItems: 'center', gap: 24,
          }}
        >
          <Card style={{ flex: 1, padding: '24px 32px', fontFamily: fSans, fontWeight: 500, fontSize: 40, color: C.navy }}>
            下一句台词
            <Cursor f={f} />
          </Card>
          <div
            style={{
              fontFamily: fSans, fontWeight: 900, fontSize: 36, padding: '12px 28px', borderRadius: 18,
              background: press ? `linear-gradient(180deg, ${C.blue}, ${C.blueLight})` : '#fff', color: press ? '#fff' : C.navy,
              boxShadow: `0 ${press ? 4 : 10}px 0 ${press ? '#0a5aa0' : C.line}`, transform: `translateY(${press ? 6 : 0}px)`,
            }}
          >
            Enter
          </div>
        </div>
        <div style={{ position: 'absolute', left: 0, top: 54, opacity: ghost, fontFamily: fSans, fontWeight: 700, fontSize: 36, color: C.blue }}>
          发送后输入框自动收起 · 按 F8 再唤出
        </div>
      </div>
    </Backdrop>
  );
};

/* ---------- stickers & images ---------- */
const CODE = ['[sticker=cats/hello h=1.6]', '[img="file:C:/meme.jpg"]'];
const ST_LINE = parseMarkup('收到啦[sticker=cats/hello]看这张图[img="file:C:/meme.jpg"]');
const ST_T = plain(ST_LINE);

export const BeatSticker: React.FC = () => {
  const n = useTyped(ST_T, 14, { base_ms: 50, cjk_factor: 1.2 }, 1);
  const card = useSpring(10);
  return (
    <Backdrop>
      <Left kicker="STICKERS & IMAGES" title="表情包与图片" sub="自己的 GIF 表情包与本地图片" />
      <Card style={{ position: 'absolute', left: 980, top: 130, width: 840, padding: '30px 40px' }}>
        {CODE.map((c, i) => (
          <CodeRow key={c} text={c} delay={4 + i * 6} />
        ))}
      </Card>
      <div style={{ position: 'absolute', left: 100, top: 600, opacity: card, transform: `translateY(${(1 - card) * 60}px)` }}>
        <ScaledSkin id="halo" width={1600} regionH={210} name="奈奈" spans={ST_LINE} visible={n} showContinue={false} />
      </div>
    </Backdrop>
  );
};

const CodeRow: React.FC<{ text: string; delay: number }> = ({ text, delay }) => {
  const p = useSpring(delay);
  return (
    <div
      style={{
        fontFamily: fMono, fontWeight: 700, fontSize: 36, color: C.blue, lineHeight: 1.8,
        opacity: p, transform: `translateX(${(1 - p) * 40}px)`, whiteSpace: 'nowrap',
      }}
    >
      {text}
    </div>
  );
};
