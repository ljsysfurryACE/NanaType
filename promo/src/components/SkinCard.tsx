import React from 'react';
import { useCurrentFrame, useVideoConfig } from 'remotion';
import skins from '../skins.generated.json';
import { fontStack } from '../theme';
import type { Span } from '../markup';
import { Sticker } from './Stickers';

export type SkinId = keyof typeof skins;
export const SKINS = skins as unknown as Record<string, any>;

type Vec = number[];
const ANCHORS: Record<string, [number, number]> = {
  top_left: [0, 0], top: [0.5, 0], top_right: [1, 0],
  left: [0, 0.5], center: [0.5, 0.5], right: [1, 0.5],
  bottom_left: [0, 1], bottom: [0.5, 1], bottom_right: [1, 1],
};

const makeCol = (s: any) => (v?: string): string | undefined => {
  if (!v) return undefined;
  const d = s.dialogue ?? {};
  const roles: Record<string, string | undefined> = {
    box: d.box?.color ?? '#ffffff',
    border: d.box?.border_color ?? '#cbe0ef',
    accent: s.chrome?.accent ?? '#0876d0',
    text: d.text?.color ?? '#1b2f4b',
    name: d.nameplate?.color ?? '#0876d0',
    name_text: d.nameplate?.text_color ?? '#ffffff',
  };
  return roles[v] ?? v;
};

const grad = (g: any, col: (v?: string) => string | undefined) => {
  if (!g) return undefined;
  const stops = g.stops.map((st: [number, string]) => `${col(st[1])} ${st[0] * 100}%`).join(', ');
  if (g.kind === 'radial') {
    const c = g.center ?? [50, 50];
    return `radial-gradient(${g.circle ? 'circle' : 'ellipse'} at ${c[0]}% ${c[1]}%, ${stops})`;
  }
  return `linear-gradient(${g.angle ?? 180}deg, ${stops})`;
};

const shadowCss = (arr: any[] | undefined, col: (v?: string) => string | undefined) =>
  (arr ?? [])
    .map((sh) => `${sh.inset ? 'inset ' : ''}${sh.dx ?? 0}px ${sh.dy ?? 0}px ${sh.blur ?? 0}px ${sh.spread ?? 0}px ${col(sh.color)}`)
    .join(', ');

const radiusCss = (o: any, fallback = 0) =>
  o.radii ? (o.radii as Vec).map((r) => `${r}px`).join(' ') : `${o.radius ?? fallback}px`;

const borderCss = (o: any, col: (v?: string) => string | undefined, defW = 0) => {
  const c = col(o.border_color) ?? 'transparent';
  const w: Vec = o.border_widths ?? [o.border_width ?? defW, o.border_width ?? defW, o.border_width ?? defW, o.border_width ?? defW];
  return {
    borderStyle: o.border_style ?? 'solid',
    borderColor: c,
    borderWidth: `${w[0]}px ${w[1]}px ${w[2]}px ${w[3]}px`,
  } as React.CSSProperties;
};

const dim = (v: number | string, total: number) => (typeof v === 'string' && v.endsWith('%') ? (parseFloat(v) / 100) * total : (v as number));

export const regionOf = (id: string): [number, number] => {
  const r = SKINS[id].dialogue?.region;
  return r ? [r[0], r[1]] : [760, 200];
};

type Props = {
  id: string;
  spans: Span[];
  visible: number;
  name?: string;
  /** frame used for continuous motion (defaults to current frame) */
  motionFrame?: number;
  style?: React.CSSProperties;
  showContinue?: boolean;
  regionH?: number;
};

export const SkinCard: React.FC<Props> = ({ id, spans, visible, name, motionFrame, style, showContinue = true, regionH }) => {
  const s = SKINS[id];
  const cur = useCurrentFrame();
  const frame = motionFrame ?? cur;
  const { fps } = useVideoConfig();
  const t = frame / fps;
  const col = makeCol(s);
  const d = s.dialogue ?? {};
  const [RW, RH0] = regionOf(id);
  const RH = regionH ?? RH0;
  const box = d.box ?? {};
  const margin: Vec = box.margin ?? [0, 0, 0, 0];
  const bw = (RW - margin[1] - margin[3]) * (box.width_fraction ?? 1);
  const bh = RH - margin[0] - margin[2];
  const bx = margin[3] + (RW - margin[1] - margin[3] - bw) / 2;
  const by = margin[0];
  const pad: Vec = d.padding ?? [20, 22, 20, 14];
  const text = d.text ?? {};
  const fx = s.effects ?? {};
  const bodyFont = fontStack(s.fonts?.body?.families);
  const nameFont = s.fonts?.name?.families ? fontStack(s.fonts.name.families) : bodyFont;
  const accentFont = s.fonts?.accent?.families ? fontStack(s.fonts.accent.families) : bodyFont;
  const bodyWeight = s.fonts?.body?.weight ?? 500;

  const glow = box.glow ? `0 0 ${box.glow_blur ?? 12}px ${col(box.glow)}` : '';
  const legacy = box.shadow ? `${box.shadow.dx ?? 0}px ${box.shadow.dy ?? 0}px ${box.shadow.blur ?? 0}px ${col(box.shadow.color)}` : '';
  const boxShadow = [shadowCss(box.shadows, col), glow, legacy].filter(Boolean).join(', ') || undefined;
  const boxColor = col(box.color ?? '#ffffff');
  const gradient = grad(box.gradient, col);

  const anchorPos = (anchor: string, ox: number, oy: number, w: number, h: number, W: number, Hh: number) => {
    const [ax, ay] = ANCHORS[anchor ?? 'top_left'] ?? [0, 0];
    return { left: ax * W + ox - ax * w, top: ay * Hh + oy - ay * h };
  };

  // ---- characters with per-char effects
  let idx = 0;
  const amp = { wave: fx.wave_px ?? 3, shake: fx.shake_px ?? 1.6, jump: fx.jump_px ?? 6, period: fx.rainbow_period_s ?? 2.4 };
  const renderSpan = (sp: Span, si: number) => {
    const chars: React.ReactNode[] = [];
    if (sp.sticker) {
      const i = idx++;
      const k = Math.max(0, Math.min(1, (visible - i) / 2));
      const sz = (text.size ?? 28) * (sp.sticker === 'img' ? 4 : 3);
      return (
        <span
          key={si}
          style={{ display: 'inline-block', margin: '0 4px', opacity: k, transform: `scale(${0.3 + 0.7 * k}) rotate(${(1 - k) * -20}deg)` }}
        >
          <Sticker kind={sp.sticker} size={sz} />
        </span>
      );
    }
    for (const ch of Array.from(sp.text)) {
      const i = idx++;
      const shown = i < visible;
      let dy = 0, dx = 0, color = col(sp.color);
      if (sp.fx?.includes('wave')) dy += Math.sin(t * Math.PI * 2 * 1.1 + i * 0.7) * amp.wave;
      if (sp.fx?.includes('jump')) dy -= Math.abs(Math.sin(t * Math.PI * 2 * 1.3 + i * 0.4)) * amp.jump;
      if (sp.fx?.includes('shake')) {
        dx += Math.sin(frame * 12.9898 + i * 78.233) * amp.shake;
        dy += Math.cos(frame * 4.1414 + i * 37.719) * amp.shake;
      }
      if (sp.fx?.includes('rainbow')) color = `hsl(${((t / amp.period) * 360 + i * 28) % 360}, 90%, 52%)`;
      chars.push(
        <span
          key={i}
          style={{
            display: 'inline-block',
            whiteSpace: 'pre',
            opacity: shown ? 1 : 0,
            transform: `translate(${dx}px, ${dy}px)`,
            color,
            fontWeight: sp.bold ? 800 : undefined,
          }}
        >
          {ch}
        </span>,
      );
    }
    if (sp.ruby) {
      return (
        <ruby key={si} style={{ rubyPosition: 'over' }}>
          {chars}
          <rt style={{ fontSize: '0.45em', letterSpacing: 0, WebkitTextStroke: 0, textShadow: 'none' }}>{sp.ruby}</rt>
        </ruby>
      );
    }
    return <React.Fragment key={si}>{chars}</React.Fragment>;
  };

  // ---- continue marker
  const cont = d.continue ?? {};
  const glyph = cont.glyph ?? '▼';
  const period = cont.period_s ?? 0.83;
  const motion = cont.motion ?? 'bob';
  const ph = (t % period) / period;
  const contStyle: React.CSSProperties = {};
  if (motion === 'bob') contStyle.transform = `translateY(${Math.sin(ph * Math.PI * 2) * (cont.bob_px ?? 3)}px)`;
  if (motion === 'blink') contStyle.opacity = ph < 0.5 ? 1 : 0.1;
  if (motion === 'pulse') contStyle.transform = `scale(${0.85 + 0.3 * (0.5 + 0.5 * Math.sin(ph * Math.PI * 2))})`;

  const decors: any[] = d.decor ?? [];
  const renderDecor = (dc: any, k: number) => {
    if (dc.when === 'speaker' && !name) return null;
    let w: number, h: number, pos: React.CSSProperties;
    if (dc.inset) {
      const ins: Vec = dc.inset;
      pos = { left: ins[0], top: ins[1], right: ins[2], bottom: ins[3] };
      w = bw - ins[0] - ins[2];
      h = bh - ins[1] - ins[3];
    } else {
      w = dim(dc.size?.[0] ?? 20, bw);
      h = dim(dc.size?.[1] ?? 20, bh);
      const p = anchorPos(dc.anchor, dc.offset?.[0] ?? 0, dc.offset?.[1] ?? 0, w, h, bw, bh);
      pos = { left: p.left, top: p.top, width: w, height: h };
    }
    const bgImg = grad(dc.gradient, col);
    const st: React.CSSProperties = {
      position: 'absolute',
      ...pos,
      boxSizing: 'border-box',
      backgroundColor: col(dc.color),
      backgroundImage: bgImg,
      borderRadius: radiusCss(dc),
      ...borderCss(dc, col),
      boxShadow: shadowCss(dc.shadows, col) || undefined,
      opacity: dc.opacity,
      transform: dc.rotate ? `rotate(${dc.rotate}deg)` : undefined,
      clipPath: dc.polygon ? `polygon(${(dc.polygon as number[][]).map((p) => `${p[0]}% ${p[1]}%`).join(', ')})` : undefined,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      zIndex: dc.layer === 'below' ? -1 : 2,
      pointerEvents: 'none',
    };
    if (!dc.border_width && !dc.border_widths) st.borderWidth = 0;
    return (
      <div key={k} style={st}>
        {dc.text ? (
          <span
            style={{
              color: col(dc.text_color) ?? col('accent'),
              fontSize: dc.text_size ?? 20,
              fontFamily: dc.font === 'name' ? nameFont : dc.font === 'body' ? bodyFont : accentFont,
              lineHeight: 1,
              whiteSpace: 'nowrap',
            }}
          >
            {dc.text}
          </span>
        ) : null}
      </div>
    );
  };

  // ---- tail
  const tail = d.tail;
  const renderTail = () => {
    if (!tail) return null;
    const side = tail.side ?? 'bottom';
    const at = tail.at ?? 0.15;
    const wd = tail.width ?? 34;
    const len = tail.length ?? 24;
    const lean = tail.lean ?? 0;
    const fill = col(tail.color) ?? boxColor ?? '#fff';
    const bc = col(box.border_color) ?? 'transparent';
    const bwid = box.border_width ?? 0;
    const sh = box.shadows?.[0];
    const filter = sh && (tail.shadow ?? true) && !sh.blur ? `drop-shadow(${sh.dx ?? 0}px ${sh.dy ?? 0}px 0 ${col(sh.color)})` : undefined;
    const pad2 = 4;
    let svgW: number, svgH: number, left: number, top: number, pts: string;
    if (side === 'bottom' || side === 'top') {
      const root = at * bw;
      svgW = wd + Math.abs(lean) + pad2 * 2;
      svgH = len + bwid + pad2 * 2;
      left = root - pad2 + Math.min(0, lean);
      const x0 = pad2 + (lean < 0 ? -lean : 0);
      const tipX = x0 + wd / 2 + lean;
      if (side === 'bottom') {
        top = bh - bwid - 1;
        pts = `${x0},${pad2} ${x0 + wd},${pad2} ${tipX},${pad2 + len + bwid}`;
      } else {
        top = -len - pad2 + 1 - bwid;
        pts = `${x0},${svgH - pad2} ${x0 + wd},${svgH - pad2} ${tipX},${pad2}`;
      }
    } else {
      const root = at * bh;
      svgH = wd + Math.abs(lean) + pad2 * 2;
      svgW = len + bwid + pad2 * 2;
      top = root - pad2 + Math.min(0, lean);
      const y0 = pad2 + (lean < 0 ? -lean : 0);
      const tipY = y0 + wd / 2 + lean;
      if (side === 'right') {
        left = bw - bwid - 1;
        pts = `${pad2},${y0} ${pad2},${y0 + wd} ${pad2 + len + bwid},${tipY}`;
      } else {
        left = -len - pad2 + 1 - bwid;
        pts = `${svgW - pad2},${y0} ${svgW - pad2},${y0 + wd} ${pad2},${tipY}`;
      }
    }
    return (
      <svg
        width={svgW}
        height={svgH}
        style={{ position: 'absolute', left, top, overflow: 'visible', zIndex: 0, filter }}
      >
        <polygon points={pts} fill={fill} stroke={bwid ? bc : 'none'} strokeWidth={bwid} strokeLinejoin="round" />
      </svg>
    );
  };

  // ---- nameplate
  const np = d.nameplate ?? {};
  const npW = 0; // measured by layout via anchor transforms
  const [nax, nay] = ANCHORS[np.anchor ?? 'top_left'] ?? [0, 0];
  const npOff: Vec = np.offset ?? [16, -12];
  const npPad: Vec = np.padding ?? [10, 4];
  void npW;
  const npStyle: React.CSSProperties = {
    position: 'absolute',
    left: nax * bw + npOff[0],
    top: nay * bh + npOff[1],
    transform: `translate(${-nax * 100}%, ${-nay * 100}%) rotate(${np.rotate ?? 0}deg)`,
    padding: `${npPad[1]}px ${npPad[0]}px`,
    background: grad(np.gradient, col) ?? col(np.color ?? '#0876d0'),
    backgroundColor: col(np.color ?? '#0876d0'),
    color: col(np.text_color ?? '#ffffff'),
    fontSize: np.size ?? 18,
    fontFamily: nameFont,
    fontWeight: s.fonts?.name?.weight ?? 700,
    borderRadius: radiusCss(np, 10),
    ...borderCss(np, col),
    boxShadow: shadowCss(np.shadows, col) || undefined,
    textShadow: np.text_shadow ? `${np.text_shadow.dx ?? 0}px ${np.text_shadow.dy ?? 0}px ${np.text_shadow.blur ?? 0}px ${col(np.text_shadow.color)}` : undefined,
    WebkitTextStroke: np.stroke ? `${np.stroke.width * 2}px ${col(np.stroke.color)}` : undefined,
    paintOrder: 'stroke fill',
    whiteSpace: 'nowrap',
    lineHeight: 1.25,
    zIndex: 3,
    boxSizing: 'border-box',
  };
  if (!np.border_width && !np.border_widths) npStyle.borderWidth = 0;
  if (np.color && np.color === '#00000000') npStyle.background = 'transparent';

  const size = text.size ?? 28;
  const align = text.align ?? 'start';

  return (
    <div style={{ position: 'relative', width: RW, height: RH, ...style }}>
      <div style={{ position: 'absolute', left: bx, top: by, width: bw, height: bh, isolation: 'isolate' }}>
        {renderTail()}
        <div
          style={{
            position: 'absolute',
            inset: 0,
            boxSizing: 'border-box',
            backgroundColor: boxColor,
            backgroundImage: gradient,
            borderRadius: radiusCss(box, 18),
            ...borderCss(box, col, 1),
            boxShadow,
            zIndex: 1,
          }}
        />
        {decors.map(renderDecor)}
        <div
          style={{
            position: 'absolute',
            left: pad[0],
            top: pad[1],
            right: pad[2],
            bottom: pad[3],
            zIndex: 2,
            color: col(text.color ?? '#1b2f4b'),
            fontSize: size,
            lineHeight: text.line_height ?? 1.4,
            fontFamily: bodyFont,
            fontWeight: bodyWeight,
            textAlign: align === 'center' ? 'center' : align === 'end' ? 'right' : 'left',
            letterSpacing: text.letter_spacing,
            WebkitTextStroke: text.stroke ? `${text.stroke.width * 2}px ${col(text.stroke.color)}` : undefined,
            paintOrder: 'stroke fill',
            textShadow: text.shadow
              ? `${text.shadow.dx ?? 0}px ${text.shadow.dy ?? 0}px ${text.shadow.blur ?? 0}px ${col(text.shadow.color)}`
              : undefined,
            overflow: 'visible',
            wordBreak: 'break-all',
          }}
        >
          {spans.map(renderSpan)}
        </div>
        {name ? <div style={npStyle}>{name}</div> : null}
        {showContinue && glyph ? (
          (() => {
            const co: Vec = cont.offset ?? [-14, -8];
            const [cax, cay] = ANCHORS[cont.anchor ?? 'bottom_right'];
            return (
              <div
                style={{
                  position: 'absolute',
                  left: cax * bw + co[0],
                  top: cay * bh + co[1],
                  transform: 'translate(-100%, -100%)',
                  zIndex: 3,
                  color: col(cont.color ?? 'accent'),
                  fontSize: cont.size ?? 16,
                  fontFamily: accentFont,
                  lineHeight: 1,
                  ...contStyle,
                }}
              >
                {glyph}
              </div>
            );
          })()
        ) : null}
      </div>
    </div>
  );
};
