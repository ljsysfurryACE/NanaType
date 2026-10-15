import React from 'react';
import { Img, interpolate, staticFile, useCurrentFrame } from 'remotion';
import { Backdrop, Card, Chip, Sparkle, Title, useSpring, useTyped } from '../components/ui';
import { ScaledSkin } from '../components/ScaledSkin';
import { parseMarkup, plain } from '../markup';
import { C, fMono, fRound, fSans } from '../theme';

const clamp = { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' } as const;
const LINE = parseMarkup('透明背景，直接入镜～');
const LINE_T = plain(LINE);

export const Output: React.FC = () => {
  const f = useCurrentFrame();
  const left = useSpring(4);
  const right = useSpring(14);
  const n = useTyped(LINE_T, 30, { base_ms: 50, cjk_factor: 1.5 }, 1.3);
  const nodes = ['VTube Studio', 'OSC', 'WebSocket'];
  return (
    <Backdrop>
      <div style={{ position: 'absolute', left: 100, top: 70 }}>
        <Title kicker="OUTPUT" title="透明直出，接入直播间" size={80} />
      </div>

      <Card style={{ position: 'absolute', left: 100, top: 300, width: 680, height: 330, padding: 24, opacity: left, transform: `translateX(${(1 - left) * -80}px)` }}>
        <div style={{ fontFamily: fRound, fontWeight: 900, fontSize: 34, color: C.blue }}>NanaType</div>
        <div style={{ marginTop: 30 }}>
          <ScaledSkin id="halo" width={620} regionH={130} name="奈奈" spans={LINE} visible={n} showContinue={false} />
        </div>
      </Card>

      {/* Spout2 link */}
      <div style={{ position: 'absolute', left: 800, top: 440, width: 300, height: 8, borderRadius: 4, background: C.line }}>
        <div style={{ position: 'absolute', inset: 0, borderRadius: 4, background: `linear-gradient(90deg, ${C.blue}, ${C.pink})`, width: `${interpolate(f, [16, 40], [0, 100], clamp)}%` }} />
        {[0, 1, 2].map((i) => (
          <div
            key={i}
            style={{
              position: 'absolute', top: -8, width: 24, height: 24, borderRadius: 12, background: '#fff', boxShadow: `0 0 16px ${C.blueLight}`,
              left: `${((f * 2.2 + i * 33) % 100)}%`, opacity: f > 30 ? 1 : 0,
            }}
          />
        ))}
        <div style={{ position: 'absolute', left: '50%', top: -70, transform: 'translateX(-50%)' }}>
          <Chip delay={20} size={34} color={C.pink}>Spout2</Chip>
        </div>
      </div>

      {/* OBS mock */}
      <div
        style={{
          position: 'absolute', left: 1120, top: 270, width: 700, height: 400, borderRadius: 24, overflow: 'hidden', background: '#2b2f38',
          boxShadow: '0 20px 56px #1b2f4b55', opacity: right, transform: `translateX(${(1 - right) * 80}px)`,
        }}
      >
        <div style={{ height: 44, background: '#1f2229', display: 'flex', alignItems: 'center', padding: '0 16px', gap: 8, fontFamily: fSans, color: '#aab', fontWeight: 700, fontSize: 20 }}>
          <span style={{ width: 12, height: 12, borderRadius: 6, background: '#ff5c7a' }} />
          <span style={{ width: 12, height: 12, borderRadius: 6, background: '#ffc85a' }} />
          <span style={{ width: 12, height: 12, borderRadius: 6, background: '#48d296' }} />
          <span style={{ marginLeft: 12 }}>OBS Studio</span>
        </div>
        <div
          style={{
            position: 'absolute', left: 0, right: 0, top: 44, bottom: 0,
            backgroundImage: 'linear-gradient(45deg,#3a3f4a 25%,transparent 25%,transparent 75%,#3a3f4a 75%),linear-gradient(45deg,#3a3f4a 25%,transparent 25%,transparent 75%,#3a3f4a 75%)',
            backgroundSize: '40px 40px', backgroundPosition: '0 0, 20px 20px', backgroundColor: '#2f333d',
          }}
        >
          <div style={{ position: 'absolute', left: 40, top: 190 }}>
            <ScaledSkin id="halo" width={620} regionH={130} name="奈奈" spans={LINE} visible={n} showContinue={false} />
          </div>
        </div>
      </div>

      {/* bridges */}
      <div style={{ position: 'absolute', left: 100, top: 780, display: 'flex', gap: 28, alignItems: 'center' }}>
        <div style={{ fontFamily: fMono, fontWeight: 700, fontSize: 30, color: C.muted }}>✦ INTEGRATIONS</div>
        {nodes.map((t, i) => (
          <NodeChip key={t} text={t} delay={46 + i * 8} />
        ))}
      </div>
      <div style={{ position: 'absolute', left: 100, top: 880, display: 'flex', gap: 20 }}>
        <Chip delay={72} size={30} color={C.navy}>窗口隐藏时也持续输出</Chip>
        <Chip delay={80} size={30} color={C.navy}>GPU 共享纹理 · 直出直播画面</Chip>
      </div>
    </Backdrop>
  );
};

const NodeChip: React.FC<{ text: string; delay: number }> = ({ text, delay }) => {
  const p = useSpring(delay);
  return (
    <div
      style={{
        fontFamily: fSans, fontWeight: 900, fontSize: 36, color: C.navy, background: '#fff', borderRadius: 18, padding: '10px 28px',
        boxShadow: '0 8px 24px #1b2f4b29, inset 0 1px 0 #fff', border: `2px solid ${C.line}`, transform: `scale(${p})`, opacity: p,
      }}
    >
      {text}
    </div>
  );
};

export const Outro: React.FC = () => {
  const icon = useSpring(0, { damping: 11, stiffness: 150 });
  const word = useSpring(8);
  const tag = useSpring(16);
  return (
    <Backdrop>
      <div style={{ position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
        <Img
          src={staticFile('nanatype-256.png')}
          style={{ width: 240, height: 240, borderRadius: 56, transform: `scale(${icon})`, boxShadow: '0 24px 56px #0876d055' }}
        />
        <div
          style={{
            fontFamily: fRound, fontWeight: 900, fontSize: 168, letterSpacing: -2, marginTop: 20,
            background: `linear-gradient(90deg, ${C.blue}, ${C.blueLight})`, WebkitBackgroundClip: 'text', color: 'transparent',
            transform: `translateY(${(1 - word) * 40}px)`, opacity: word,
          }}
        >
          NanaType
        </div>
        <div style={{ fontFamily: fRound, fontWeight: 700, fontSize: 48, color: C.muted, opacity: tag }}>Typewriter for VTubers</div>
        <div style={{ display: 'flex', gap: 20, marginTop: 40 }}>
          <Chip delay={26} size={34}>Windows · macOS</Chip>
          <Chip delay={32} size={34}>中文 / EN / 日本語</Chip>
          <Chip delay={38} size={34} color={C.pink}>MIT / Apache-2.0</Chip>
        </div>
        <div style={{ fontFamily: fMono, fontWeight: 700, fontSize: 34, color: C.navy, marginTop: 44, opacity: useSpring(46) }}>
          <Sparkle size={26} color={C.pink} style={{ marginRight: 12 }} />
          github.com/sena-nana/NanaType
        </div>
      </div>
    </Backdrop>
  );
};
