import React from 'react';
import { SkinCard, regionOf } from './SkinCard';
import type { Span } from '../markup';

export const ScaledSkin: React.FC<{
  id: string;
  spans: Span[];
  visible: number;
  name?: string;
  scale?: number;
  width?: number;
  style?: React.CSSProperties;
  showContinue?: boolean;
  motionFrame?: number;
  regionH?: number;
}> = ({ id, spans, visible, name, scale, width, style, showContinue, motionFrame, regionH }) => {
  const [rw, rh0] = regionOf(id);
  const rh = regionH ?? rh0;
  const s = scale ?? (width ? width / rw : 1);
  return (
    <div style={{ width: rw * s, height: rh * s, position: 'relative', ...style }}>
      <div style={{ transform: `scale(${s})`, transformOrigin: 'top left', position: 'absolute', left: 0, top: 0 }}>
        <SkinCard id={id} spans={spans} visible={visible} name={name} showContinue={showContinue} motionFrame={motionFrame} regionH={regionH} />
      </div>
    </div>
  );
};
