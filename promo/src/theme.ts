import { loadFont as loadSans } from '@remotion/google-fonts/NotoSansSC';
import { loadFont as loadSerif } from '@remotion/google-fonts/NotoSerifSC';
import { loadFont as loadKai } from '@remotion/google-fonts/LXGWWenKaiTC';
import { loadFont as loadPixel } from '@remotion/google-fonts/DotGothic16';
import { loadFont as loadMono } from '@remotion/google-fonts/JetBrainsMono';
import { loadFont as loadRound } from '@remotion/google-fonts/ZenMaruGothic';

const opts = { ignoreTooManyRequestsWarning: true } as const;
export const fSans = loadSans('normal', { weights: ['500', '700', '900'], ...opts }).fontFamily;
export const fSerif = loadSerif('normal', { weights: ['700'], ...opts }).fontFamily;
export const fKai = loadKai('normal', { weights: ['400'], ...opts }).fontFamily;
export const fPixel = loadPixel('normal', { weights: ['400'], ...opts }).fontFamily;
export const fMono = loadMono('normal', { weights: ['400', '700'], ...opts }).fontFamily;
export const fRound = loadRound('normal', { weights: ['500', '700', '900'], ...opts }).fontFamily;

/** Nana series tokens (NanaUI look.md / halo.toml / manifest.rs) */
export const C = {
  blue: '#0876d0',
  blueLight: '#3aa6f0',
  blueBrand: '#1d82fa',
  sky: '#8fd3ff',
  ice: '#eaf4fb',
  white: '#ffffff',
  navy: '#1b2f4b',
  muted: '#4a6280',
  line: '#cbe0ef',
  pink: '#e86a92',
  dark: '#1a2233',
};
export const SHADOW = '0 8px 24px #1b2f4b29, inset 0 1px 0 #ffffff';
export const SHADOW_LG = '0 18px 48px #1b2f4b33, inset 0 1px 0 #ffffff';
export const FPS = 30;
export const W = 1920;
export const H = 1080;

/** map a skin font family name to a loaded CSS family */
export const fontStack = (families: string[] | undefined): string => {
  const f = (families?.[0] ?? '').toLowerCase();
  if (f.includes('serif') || f.includes('song') || f.includes('宋')) return `${fSerif}, serif`;
  if (f.includes('wenkai') || f.includes('kai')) return `${fKai}, serif`;
  if (f.includes('pixel') || f.includes('zpix') || f.includes('gothic')) return `${fPixel}, monospace`;
  if (f.includes('mono') || f.includes('consolas') || f.includes('cascadia') || f.includes('sarasa')) return `${fMono}, ${fSans}, monospace`;
  return `${fSans}, sans-serif`;
};
