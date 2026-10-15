import React from 'react';
import { interpolate, useCurrentFrame } from 'remotion';
import { Backdrop, Chip, KeyCap, Title, useSpring, useTyped } from '../components/ui';
import { ScaledSkin } from '../components/ScaledSkin';
import { parseMarkup, plain } from '../markup';
import { C, fSans } from '../theme';

const L1 = parseMarkup('大家好～我是奈奈！今天也要[rainbow][wave]元气满满[/wave][/rainbow]地[shake]开播[/shake]！');
const L2 = parseMarkup('[jump]马上[/jump]就开始啦～记得[color=#e86a92]点关注[/color]哦！');
const T1 = plain(L1);
const T2 = plain(L2);

const HistoryRow: React.FC<{ delay: number; text: string; time: string }> = ({ delay, text, time }) => {
  const p = useSpring(delay);
  return (
    <div style={{ display: 'flex', gap: 14, alignItems: 'flex-start', opacity: p, transform: `translateY(${(1 - p) * 50}px)` }}>
      <div
        style={{
          width: 56, height: 56, borderRadius: 28, flex: 'none',
          background: `linear-gradient(135deg, ${C.sky}, ${C.pink})`, border: '3px solid #fff',
          boxShadow: '0 3px 8px #1b2f4b33', color: '#fff', fontWeight: 900, fontSize: 26,
          display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: fSans,
        }}
      >
        奈
      </div>
      <div>
        <div style={{ display: 'flex', gap: 10, alignItems: 'center', fontFamily: fSans }}>
          <span style={{ background: `linear-gradient(90deg, ${C.blue}, ${C.blueLight})`, color: '#fff', borderRadius: 999, padding: '2px 14px', fontWeight: 700, fontSize: 22 }}>奈奈</span>
          <span style={{ color: C.muted, fontSize: 20 }}>{time}</span>
        </div>
        <div
          style={{
            marginTop: 8, background: '#fff', borderRadius: '4px 18px 18px 18px', padding: '12px 18px', fontSize: 28,
            fontFamily: fSans, fontWeight: 500, color: C.navy, boxShadow: '0 4px 12px #1b2f4b22', width: 340,
          }}
        >
          {text}
        </div>
      </div>
    </div>
  );
};

export const Typewriter: React.FC = () => {
  const f = useCurrentFrame();
  const stage = useSpring(0, { damping: 18 });
  const second = f >= 126;
  const n1 = useTyped(T1, 14, { base_ms: 50, cjk_factor: 1.5 }, 1.1);
  const n2 = useTyped(T2, 130, { base_ms: 50, cjk_factor: 1.5 }, 1.1);
  const key = f >= 118 && f < 132;
  const kp = interpolate(f, [112, 118], [0, 1], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
  return (
    <Backdrop>
      <div style={{ position: 'absolute', left: 100, top: 70 }}>
        <Title kicker="TYPEWRITER" title="逐字呈现，像演出一样" size={80} />
      </div>
      <div style={{ position: 'absolute', left: 100, top: 220, display: 'flex', gap: 16 }}>
        <Chip delay={14} size={30}>标点停顿</Chip>
        <Chip delay={20} size={30}>中日文节奏自适应</Chip>
        <Chip delay={26} size={30} color={C.pink}>▼ 等待继续</Chip>
      </div>

      <div
        style={{
          position: 'absolute', left: 100, top: 330, width: 1240, height: 640, borderRadius: 32, overflow: 'hidden',
          background: 'linear-gradient(135deg, #cfe8ff 0%, #e9ddff 55%, #ffe3ee 100%)',
          boxShadow: '0 20px 56px #1b2f4b33', transform: `scale(${0.9 + stage * 0.1})`, opacity: stage,
        }}
      >
        <div style={{ position: 'absolute', left: 60, top: 50, width: 260, height: 260, borderRadius: 130, background: '#ffffff88' }} />
        <div style={{ position: 'absolute', right: 80, top: 120, width: 180, height: 180, borderRadius: 90, background: '#8fd3ff66' }} />
        <div style={{ position: 'absolute', left: 20, top: 20, background: '#ff5c7a', color: '#fff', fontFamily: fSans, fontWeight: 900, fontSize: 22, borderRadius: 999, padding: '4px 16px' }}>● LIVE</div>
        <div style={{ position: 'absolute', left: 40, bottom: 30 }}>
          <ScaledSkin
            id="halo"
            width={1160}
            regionH={150}
            name="奈奈"
            spans={second ? L2 : L1}
            visible={second ? n2 : n1}
            showContinue={second ? n2 >= T2.length : n1 >= T1.length}
          />
        </div>
      </div>

      <div style={{ position: 'absolute', left: 1400, top: 330, display: 'flex', flexDirection: 'column', gap: 28 }}>
        <HistoryRow delay={40} text="今天玩什么游戏呢？" time="20:01" />
        <HistoryRow delay={56} text="先聊聊天，再开冲！" time="20:02" />
        <HistoryRow delay={132} text="大家好～我是奈奈！" time="20:03" />
      </div>
      <div style={{ position: 'absolute', left: 1400, top: 240 }}>
        <Chip delay={30} size={28} color={C.navy}>History · 历史记录</Chip>
      </div>

      <div style={{ position: 'absolute', left: 1500, top: 800, transform: 'scale(0.5)', transformOrigin: 'top left', opacity: kp }}>
        <KeyCap label="Enter" pressed={key} hint="发送" />
      </div>
    </Backdrop>
  );
};
