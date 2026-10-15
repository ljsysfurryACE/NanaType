/** Tiny subset of NanaType markup: [wave] [shake] [jump] [rainbow] [color=#hex] [ruby=xx] [b] */
export type Span = {
  text: string;
  fx?: ('wave' | 'shake' | 'jump' | 'rainbow')[];
  color?: string;
  ruby?: string;
  bold?: boolean;
  /** inline sticker / emote / image stand-in: 'cat' | 'wave' | 'img' */
  sticker?: string;
};

export const parseMarkup = (src: string): Span[] => {
  const out: Span[] = [];
  const stack: { tag: string; arg?: string }[] = [];
  const re = /\[(sticker|emote|img)=[^\]]*\]|\[(\/)?(wave|shake|jump|rainbow|color|ruby|b)(?:=([^\]]+))?\]/g;
  let last = 0;
  const push = (text: string) => {
    if (!text) return;
    const fx = stack.filter((s) => ['wave', 'shake', 'jump', 'rainbow'].includes(s.tag)).map((s) => s.tag) as Span['fx'];
    out.push({
      text,
      fx: fx && fx.length ? fx : undefined,
      color: stack.filter((s) => s.tag === 'color').pop()?.arg,
      ruby: stack.filter((s) => s.tag === 'ruby').pop()?.arg,
      bold: stack.some((s) => s.tag === 'b') || undefined,
    });
  };
  let m: RegExpExecArray | null;
  while ((m = re.exec(src))) {
    push(src.slice(last, m.index));
    last = m.index + m[0].length;
    if (m[1]) {
      out.push({ text: '□', sticker: m[1] === 'emote' || /wave/.test(m[0]) ? 'wave' : m[1] === 'img' ? 'img' : 'cat' });
    } else if (m[2]) {
      const i = stack.map((s) => s.tag).lastIndexOf(m[3]);
      if (i >= 0) stack.splice(i, 1);
    } else stack.push({ tag: m[3], arg: m[4] });
  }
  push(src.slice(last));
  return out;
};

export const plain = (spans: Span[]) => spans.map((s) => s.text).join('');

const isCjk = (c: string) => /[⺀-鿿豈-﫿＀-￯　-〿]/.test(c);
const PUNCT_COMMA = /[,，、;；]/;
const PUNCT_STOP = /[.。!！?？]/;

export type Reveal = { base_ms?: number; cjk_factor?: number; punct?: { comma_ms?: number; stop_ms?: number; ellipsis_ms?: number } };

/** cumulative ms at which each character becomes visible */
export const charTimes = (text: string, r: Reveal = {}, speed = 1): number[] => {
  const base = r.base_ms ?? 50;
  const f = r.cjk_factor ?? 2;
  const p = { comma_ms: 120, stop_ms: 260, ellipsis_ms: 300, ...(r.punct ?? {}) };
  let t = 0;
  const out: number[] = [];
  for (const c of text) {
    if (c !== ' ' && c !== '\n') t += (isCjk(c) ? base * f : base) / speed;
    out.push(t);
    if (PUNCT_COMMA.test(c)) t += p.comma_ms / speed;
    else if (PUNCT_STOP.test(c)) t += p.stop_ms / speed;
    else if (c === '…') t += p.ellipsis_ms / speed;
  }
  return out;
};

export const visibleCount = (times: number[], ms: number) => {
  let n = 0;
  while (n < times.length && times[n] <= ms) n++;
  return n;
};
