import React from 'react';
import { AbsoluteFill } from 'remotion';
import { SKINS, SkinCard } from './components/SkinCard';
import { parseMarkup, plain } from './markup';

const order = Object.keys(SKINS);
const spans = parseMarkup('今天也请多关照！[wave]波浪[/wave]与[color=#e86a92]粉色[/color]');
export const SkinSheet: React.FC = () => (
  <AbsoluteFill style={{ background: '#7d8aa3', flexWrap: 'wrap', flexDirection: 'row', padding: 20, gap: 14 }}>
    {order.map((id) => (
      <div key={id} style={{ width: 600, height: 300, position: 'relative', background: '#c9d3e6', overflow: 'hidden' }}>
        <div style={{ transform: 'scale(0.75)', transformOrigin: 'top left', padding: 24 }}>
          <SkinCard id={id} spans={spans} visible={plain(spans).length} name="奈奈" />
        </div>
        <div style={{ position: 'absolute', right: 6, bottom: 4, fontSize: 14, color: '#223' }}>{id}</div>
      </div>
    ))}
  </AbsoluteFill>
);
