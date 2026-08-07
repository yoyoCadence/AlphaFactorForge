// AlphaFactorForge — 介面皮膚 (skin) token contract.
//
// Drop-in target: alpha-factor-forge/src/theme/theme.ts
//
// One Theme = every value the UI is allowed to hard-code. Nothing in
// src/components/* may contain a literal colour, font stack, radius, or border
// width after the skin port; it reads a token from here (via useTheme() +
// makeStyles(), or via the CSS custom properties emitted by themeToCssVars()).
//
// Skins differ on purpose in more than colour: density (`pad`/`gap`), geometry
// (`radius`/`borderWidth`/`borderStyle`), type (`sans`/`mono`/label casing),
// elevation (`shadow`), and control shape (`tab`/`chip` state pairs). That is
// what keeps them from looking like six recolours of the same screen.

import auroraBackgroundUrl from '../assets/theme-backgrounds/aurora-glass-v1.webp';
import paperBackgroundUrl from '../assets/theme-backgrounds/paper-fiber-v1.webp';
import signalOrangeBackgroundUrl from '../assets/theme-backgrounds/signal-orange-v1.webp';

export type SkinId =
  | 'forge-paper'
  | 'midnight-tape'
  | 'swiss-forge'
  | 'atelier-warm'
  | 'blueprint'
  | 'signal-orange'
  | 'broadsheet'
  | 'brutal-yellow'
  | 'frost-grey'
  | 'aurora-glass';

/** Visual state pair for a two-state control (tab / chip / toggle button). */
export interface StatePair {
  bg: string;
  fg: string;
  /** Full CSS border shorthand, e.g. '1px solid #27353d'. */
  bd: string;
  /** Bottom-border shorthand — the underline-style tabs use only this. */
  ub?: string;
}

/** Canvas drawing contract for ChartSection / SweepSection (no CSS involved). */
export interface ChartTheme {
  bg: string;
  grid: string;
  /** setLineDash pattern for grid lines; null = solid. */
  dash: number[] | null;
  axis: string;
  label: string;
  up: string;
  down: string;
  /** How a bar is drawn. Not cosmetic — each skin picks one. */
  style: 'filled' | 'hollow' | 'bar' | 'outline' | 'soft' | 'stick' | 'block' | 'line' | 'area';
  ma1: string;
  ma2: string;
  /** EMA overlay. A third line colour, so MA fast / MA slow / EMA stay tellable
   *  apart when all three are on; must also avoid `up` / `down`. */
  ema: string;
  /** Bollinger envelope (upper / middle / lower share it). Context rather than
   *  signal: quieter than the MA lines, but stronger than `grid`. */
  bb: string;
  vol: string;
  rsi: string;
  /** Last-price tag + heatmap ramp target. */
  accent: string;
  accentInk: string;
  /** Heatmap ramp origin (cold cell). */
  surface2: string;
  /** Base line width for MA / candle outlines. */
  lw: number;
}

/** Workspace-only background layers. `color.bg` remains an opaque colour so
 * contrast checks and canvas fallbacks never need to interpret an image. */
export interface WorkspaceBackgroundTheme {
  /** A local raster URL, CSS gradient/pattern, or `none`. */
  image: string;
  /** Readability layer painted above `image`; `none` for intentionally flat skins. */
  scrim: string;
  position: string;
  size: string;
  repeat: string;
}

export interface Theme {
  id: SkinId;
  /** Display name shown in the skin picker (zh-TW). */
  name: string;
  mode: 'light' | 'dark';
  workspaceBackground: WorkspaceBackgroundTheme;

  color: {
    bg: string;
    surface: string;
    surface2: string;
    cardBg: string;
    ink: string;
    muted: string;
    faint: string;
    line: string;
    line2: string;
    accent: string;
    accentInk: string;
    /** Field background marking a value the parameter sweep set. A low-intensity
     *  accent tint over `shape.fieldBg`, so `ink` stays readable on top of it —
     *  a pale blue wash would be wrong on the dark skins. */
    accentWash: string;
    ok: string;
    warn: string;
    danger: string;
  };

  font: {
    sans: string;
    mono: string;
    /** Body / value size in px. */
    size: number;
    labelSize: number;
    titleSize: number;
    labelTransform: 'none' | 'uppercase';
    labelTracking: string;
    labelWeight: number;
    titleTransform: 'none' | 'uppercase';
    titleTracking: string;
    titleWeight: number;
  };

  shape: {
    radius: number;
    radiusSm: number;
    borderWidth: number;
    borderStyle: 'solid' | 'dashed';
    /** Card padding, px. */
    pad: number;
    /** Grid gap between cards, px. */
    gap: number;
    shadow: string;
    /** backdrop-filter for translucent skins ('none' everywhere else). */
    backdrop?: string;
    fieldBg: string;
    fieldRadius: number;
    btnRadius: number;
    btnPad: string;
    btnTransform: 'none' | 'uppercase';
    btnWeight: number;
    btnShadow: string;
    chipRadius: number;
    tabRadius: number;
    rowLine: string;
  };

  button: {
    primaryBg: string;
    primaryInk: string;
    ghostBg: string;
    ghostInk: string;
    ghostLine: string;
  };

  header: {
    bg: string;
    ink: string;
    muted: string;
    height: number;
    border: string;
    markBg: string;
    markRadius: string;
    markRotate: string;
  };

  /** Tab strip track (segmented skins fill it; underline skins rule it). */
  track: { bg: string; pad: number; radius: number; gap: number; underline: string };

  states: { tab: { on: StatePair; off: StatePair }; chip: { on: StatePair; off: StatePair } };

  /** 'inline' shows the 01/02/03 section numerals (Swiss / terminal / blueprint). */
  sectionIndex: 'none' | 'inline';

  chart: ChartTheme;
}

// Each skin pairs a Latin face with a Chinese face chosen for the same voice —
// the UI is mostly zh-TW, so a shared CJK fallback would make every skin read
// identically no matter what the Latin stack says.
const CJK = {
  /** Neutral humanist gothic — the current app's voice. */
  sans: "'Noto Sans TC'",
  /** Song / 宋體 — engineering-drawing annotation face. */
  serif: "'Noto Serif TC'",
  /** Kai / 楷體 — warm, hand-cut. */
  kai: "'LXGW WenKai TC'",
  /** Mechanical HK grotesque, tighter than Noto. */
  hei: "'Chiron Hei HK'",
  /** Modern Song with high contrast — editorial headlines. */
  sung: "'Chiron Sung HK'",
  /** Rounded gothic — industrial signage / HMI. */
  round: "'Chiron GoRound TC'",
  /** 芫荽 — hand-cut TC face; newsprint column voice.
   *  (Noto Serif HK was rejected here: its glyphs are byte-identical to
   *  Noto Serif TC for this UI's character set, so the two skins looked the
   *  same. Any new CJK pick must be rasterised against the existing ones.) */
  songHK: "'Iansui'",
  /** Heaviest available CJK gothic (900) — poster / brutalist. */
  heavy: "'Noto Sans HK'",
  /** 粉圓 — soft rounded, pairs with translucent surfaces. */
  soft: "'Huninn'",
};

const flatWorkspaceBackground: WorkspaceBackgroundTheme = {
  image: 'none',
  scrim: 'none',
  position: 'center',
  size: 'auto',
  repeat: 'no-repeat',
};

function rasterWorkspaceBackground(imageUrl: string, scrim: string): WorkspaceBackgroundTheme {
  return {
    image: `url("${imageUrl}")`,
    scrim,
    position: 'center, center',
    size: 'auto, cover',
    repeat: 'no-repeat, no-repeat',
  };
}

const midnightWorkspaceBackground: WorkspaceBackgroundTheme = {
  image: 'repeating-linear-gradient(0deg, rgba(53,224,138,0.028) 0, rgba(53,224,138,0.028) 1px, transparent 1px, transparent 4px)',
  scrim: 'linear-gradient(rgba(6,8,10,0.6), rgba(6,8,10,0.72))',
  position: 'center, center',
  size: 'auto, 100% 4px',
  repeat: 'no-repeat, repeat',
};

const blueprintWorkspaceBackground: WorkspaceBackgroundTheme = {
  image: 'linear-gradient(rgba(99,216,255,0.045) 1px, transparent 1px), linear-gradient(90deg, rgba(99,216,255,0.045) 1px, transparent 1px)',
  scrim: 'linear-gradient(rgba(11,34,51,0.5), rgba(11,34,51,0.62))',
  position: 'center, center, center',
  size: 'auto, 32px 32px, 32px 32px',
  repeat: 'no-repeat, repeat, repeat',
};

export const forgePaper: Theme = {
  id: 'forge-paper',
  name: '紙感工坊',
  mode: 'light',
  workspaceBackground: rasterWorkspaceBackground(
    paperBackgroundUrl,
    'linear-gradient(rgba(236,234,228,0.76), rgba(236,234,228,0.84))',
  ),
  color: {
    bg: '#eceae4', surface: '#ffffff', surface2: '#f4f1ea', cardBg: '#ffffff',
    ink: '#16150f', muted: '#6d6a5f', faint: '#979388', line: '#d6d2c8', line2: '#b8b3a6',
    accent: '#16150f', accentInk: '#ffffff', accentWash: '#e3e3e2', ok: '#1f7a57', warn: '#8a7a3a', danger: '#b23b2e',
  },
  font: {
    sans: `'IBM Plex Sans',${CJK.sans},system-ui,sans-serif`,
    mono: `'IBM Plex Mono',${CJK.sans},ui-monospace,monospace`,
    size: 12, labelSize: 10, titleSize: 12,
    labelTransform: 'none', labelTracking: '0.04em', labelWeight: 500,
    titleTransform: 'none', titleTracking: '0.04em', titleWeight: 700,
  },
  shape: {
    radius: 0, radiusSm: 0, borderWidth: 1, borderStyle: 'solid', pad: 12, gap: 12, shadow: 'none',
    fieldBg: '#ffffff', fieldRadius: 0, btnRadius: 0, btnPad: '6px 10px', btnTransform: 'none',
    btnWeight: 600, btnShadow: '0 1px 0 rgba(22,21,15,.18)', chipRadius: 0, tabRadius: 0, rowLine: '#efece5',
  },
  button: { primaryBg: '#16150f', primaryInk: '#ffffff', ghostBg: '#efece5', ghostInk: '#16150f', ghostLine: '#d6d2c8' },
  header: {
    bg: '#ffffff', ink: '#16150f', muted: '#6d6a5f', height: 50, border: '#d6d2c8',
    markBg: '#16150f', markRadius: '0px', markRotate: '45deg',
  },
  track: { bg: 'transparent', pad: 0, radius: 0, gap: 0, underline: '1px solid #d6d2c8' },
  states: {
    tab: {
      on: { bg: 'transparent', fg: '#16150f', bd: '0px solid transparent', ub: '2px solid #16150f' },
      off: { bg: 'transparent', fg: '#8a8678', bd: '0px solid transparent', ub: '2px solid transparent' },
    },
    chip: {
      on: { bg: '#16150f', fg: '#ffffff', bd: '#16150f' },
      off: { bg: '#efece5', fg: '#8a8678', bd: '#d6d2c8' },
    },
  },
  sectionIndex: 'none',
  chart: {
    bg: '#ffffff', grid: '#efece5', dash: null, axis: '#d6d2c8', label: '#6d6a5f',
    up: '#2d9f73', down: '#d23b2f', style: 'filled', ma1: '#3a6ea5', ma2: '#d08a2c', ema: '#7a5ea8', bb: '#b9b4a8',
    vol: '#d6d2c8', rsi: '#8a8678', accent: '#16150f', accentInk: '#ffffff', surface2: '#f4f1ea', lw: 1.4,
  },
};

export const midnightTape: Theme = {
  id: 'midnight-tape',
  name: '午夜行情帶',
  mode: 'dark',
  workspaceBackground: midnightWorkspaceBackground,
  color: {
    bg: '#06080a', surface: '#0d1114', surface2: '#131a1f', cardBg: '#0d1114',
    ink: '#cfe3d8', muted: '#6f8a7e', faint: '#51675e', line: '#1a242a', line2: '#27353d',
    accent: '#35e08a', accentInk: '#04140c', accentWash: '#0c2219', ok: '#35e08a', warn: '#e8c34a', danger: '#ff5b52',
  },
  font: {
    sans: `'JetBrains Mono',${CJK.hei},monospace`,
    mono: `'JetBrains Mono',${CJK.hei},monospace`,
    size: 12, labelSize: 9, titleSize: 11,
    labelTransform: 'uppercase', labelTracking: '0.14em', labelWeight: 500,
    titleTransform: 'uppercase', titleTracking: '0.2em', titleWeight: 700,
  },
  shape: {
    radius: 0, radiusSm: 0, borderWidth: 1, borderStyle: 'solid', pad: 10, gap: 8,
    shadow: '0 0 0 1px rgba(53,224,138,.04)', fieldBg: '#06080a', fieldRadius: 0, btnRadius: 0,
    btnPad: '6px 12px', btnTransform: 'uppercase', btnWeight: 700, btnShadow: 'none',
    chipRadius: 0, tabRadius: 0, rowLine: '#131a1f',
  },
  button: { primaryBg: '#35e08a', primaryInk: '#04140c', ghostBg: 'transparent', ghostInk: '#35e08a', ghostLine: '#27353d' },
  header: {
    bg: '#0d1114', ink: '#cfe3d8', muted: '#6f8a7e', height: 46, border: '#1a242a',
    markBg: '#35e08a', markRadius: '0px', markRotate: '0deg',
  },
  track: { bg: 'transparent', pad: 0, radius: 0, gap: 6, underline: '0px solid transparent' },
  states: {
    tab: {
      on: { bg: '#35e08a', fg: '#04140c', bd: '1px solid #35e08a', ub: '1px solid #35e08a' },
      off: { bg: 'transparent', fg: '#6f8a7e', bd: '1px solid #27353d', ub: '1px solid #27353d' },
    },
    chip: {
      on: { bg: '#35e08a', fg: '#04140c', bd: '#35e08a' },
      off: { bg: 'transparent', fg: '#6f8a7e', bd: '#27353d' },
    },
  },
  sectionIndex: 'inline',
  chart: {
    bg: '#06080a', grid: '#131a1f', dash: [2, 4], axis: '#1a242a', label: '#6f8a7e',
    up: '#35e08a', down: '#ff5b52', style: 'hollow', ma1: '#4bd6ff', ma2: '#e8c34a', ema: '#c792ea', bb: '#3f5a52',
    vol: '#1d3a2c', rsi: '#35e08a', accent: '#35e08a', accentInk: '#04140c', surface2: '#131a1f', lw: 1.2,
  },
};

export const swissForge: Theme = {
  id: 'swiss-forge',
  name: '瑞士方格',
  mode: 'light',
  workspaceBackground: flatWorkspaceBackground,
  color: {
    bg: '#ffffff', surface: '#ffffff', surface2: '#f1f0ec', cardBg: '#ffffff',
    ink: '#0a0a0a', muted: '#6f6f6b', faint: '#93938e', line: '#0a0a0a', line2: '#0a0a0a',
    accent: '#e63329', accentInk: '#ffffff', accentWash: '#fce7e5', ok: '#147a4e', warn: '#a8720a', danger: '#e63329',
  },
  font: {
    sans: `'Space Grotesk',${CJK.sung},sans-serif`,
    mono: `'IBM Plex Mono',${CJK.sung},monospace`,
    size: 13, labelSize: 10, titleSize: 15,
    labelTransform: 'uppercase', labelTracking: '0.16em', labelWeight: 700,
    titleTransform: 'uppercase', titleTracking: '0.02em', titleWeight: 700,
  },
  shape: {
    radius: 0, radiusSm: 0, borderWidth: 2, borderStyle: 'solid', pad: 18, gap: 18, shadow: 'none',
    fieldBg: '#ffffff', fieldRadius: 0, btnRadius: 0, btnPad: '9px 14px', btnTransform: 'uppercase',
    btnWeight: 700, btnShadow: 'none', chipRadius: 0, tabRadius: 0, rowLine: '#dcdbd6',
  },
  button: { primaryBg: '#e63329', primaryInk: '#ffffff', ghostBg: '#ffffff', ghostInk: '#0a0a0a', ghostLine: '#0a0a0a' },
  header: {
    bg: '#0a0a0a', ink: '#ffffff', muted: '#9a9a96', height: 64, border: '#0a0a0a',
    markBg: '#e63329', markRadius: '50%', markRotate: '0deg',
  },
  track: { bg: 'transparent', pad: 0, radius: 0, gap: 0, underline: '2px solid #0a0a0a' },
  states: {
    tab: {
      on: { bg: '#0a0a0a', fg: '#ffffff', bd: '0px solid transparent', ub: '0px solid transparent' },
      off: { bg: 'transparent', fg: '#6f6f6b', bd: '0px solid transparent', ub: '0px solid transparent' },
    },
    chip: {
      on: { bg: '#e63329', fg: '#ffffff', bd: '#e63329' },
      off: { bg: '#ffffff', fg: '#0a0a0a', bd: '#0a0a0a' },
    },
  },
  sectionIndex: 'inline',
  chart: {
    bg: '#ffffff', grid: '#e2e1dc', dash: null, axis: '#0a0a0a', label: '#0a0a0a',
    up: '#0a0a0a', down: '#e63329', style: 'bar', ma1: '#0a0a0a', ma2: '#e63329', ema: '#2b4c9b', bb: '#c9c8c3',
    vol: '#dcdbd6', rsi: '#0a0a0a', accent: '#e63329', accentInk: '#ffffff', surface2: '#f1f0ec', lw: 1.6,
  },
};

export const atelierWarm: Theme = {
  id: 'atelier-warm',
  name: '暖調工作室',
  mode: 'light',
  workspaceBackground: rasterWorkspaceBackground(
    paperBackgroundUrl,
    'linear-gradient(rgba(244,241,236,0.82), rgba(244,241,236,0.9))',
  ),
  color: {
    bg: '#f4f1ec', surface: '#fffdfa', surface2: '#efe9e0', cardBg: '#fffdfa',
    ink: '#2b2925', muted: '#746d62', faint: '#979084', line: '#e7e0d4', line2: '#d5ccbc',
    accent: '#5f7a63', accentInk: '#ffffff', accentWash: '#e4e4dc', ok: '#4d7a5b', warn: '#a8792c', danger: '#b4573c',
  },
  font: {
    sans: `'Manrope',${CJK.kai},sans-serif`,
    mono: `'IBM Plex Mono',${CJK.kai},monospace`,
    size: 13, labelSize: 11, titleSize: 14,
    labelTransform: 'none', labelTracking: '0.01em', labelWeight: 600,
    titleTransform: 'none', titleTracking: '-0.01em', titleWeight: 700,
  },
  shape: {
    radius: 14, radiusSm: 10, borderWidth: 1, borderStyle: 'solid', pad: 20, gap: 16,
    shadow: '0 1px 2px rgba(43,41,37,.05), 0 16px 32px -20px rgba(43,41,37,.4)',
    fieldBg: '#f6f2ec', fieldRadius: 10, btnRadius: 999, btnPad: '8px 16px', btnTransform: 'none',
    btnWeight: 600, btnShadow: '0 1px 2px rgba(43,41,37,.10)', chipRadius: 999, tabRadius: 999, rowLine: '#f0ebe2',
  },
  button: { primaryBg: '#5f7a63', primaryInk: '#ffffff', ghostBg: '#efe9e0', ghostInk: '#2b2925', ghostLine: '#e2dad0' },
  header: {
    bg: '#fffdfa', ink: '#2b2925', muted: '#746d62', height: 62, border: '#e7e0d4',
    markBg: '#5f7a63', markRadius: '999px', markRotate: '0deg',
  },
  track: { bg: '#efe9e0', pad: 4, radius: 999, gap: 4, underline: '0px solid transparent' },
  states: {
    tab: {
      on: { bg: '#fffdfa', fg: '#2b2925', bd: '0px solid transparent', ub: '0px solid transparent' },
      off: { bg: 'transparent', fg: '#877f72', bd: '0px solid transparent', ub: '0px solid transparent' },
    },
    chip: {
      on: { bg: '#5f7a63', fg: '#ffffff', bd: '#5f7a63' },
      off: { bg: '#efe9e0', fg: '#877f72', bd: '#e2dad0' },
    },
  },
  sectionIndex: 'none',
  chart: {
    bg: '#fffdfa', grid: '#f0ebe2', dash: null, axis: '#e7e0d4', label: '#746d62',
    up: '#5f7a63', down: '#b4573c', style: 'soft', ma1: '#7a8fa6', ma2: '#c08a52', ema: '#6b4a8f', bb: '#d8cfc0',
    vol: '#ece5da', rsi: '#877f72', accent: '#5f7a63', accentInk: '#ffffff', surface2: '#efe9e0', lw: 1.8,
  },
};

export const blueprint: Theme = {
  id: 'blueprint',
  name: '藍圖製圖',
  mode: 'dark',
  workspaceBackground: blueprintWorkspaceBackground,
  color: {
    bg: '#0b2233', surface: '#0f2b40', surface2: '#143349', cardBg: '#0f2b40',
    ink: '#d9ecf8', muted: '#7fabc8', faint: '#55809e', line: '#245c7d', line2: '#2f7297',
    accent: '#63d8ff', accentInk: '#062130', accentWash: '#16384b', ok: '#7ef0c0', warn: '#ffd479', danger: '#ff8f6b',
  },
  font: {
    sans: `'IBM Plex Mono',${CJK.serif},monospace`,
    mono: `'IBM Plex Mono',${CJK.serif},monospace`,
    size: 12, labelSize: 9, titleSize: 11,
    labelTransform: 'uppercase', labelTracking: '0.18em', labelWeight: 500,
    titleTransform: 'uppercase', titleTracking: '0.22em', titleWeight: 600,
  },
  shape: {
    radius: 0, radiusSm: 0, borderWidth: 1, borderStyle: 'dashed', pad: 14, gap: 12, shadow: 'none',
    fieldBg: '#0b2233', fieldRadius: 0, btnRadius: 0, btnPad: '7px 12px', btnTransform: 'uppercase',
    btnWeight: 600, btnShadow: 'none', chipRadius: 0, tabRadius: 0, rowLine: '#143349',
  },
  button: { primaryBg: '#63d8ff', primaryInk: '#062130', ghostBg: 'transparent', ghostInk: '#63d8ff', ghostLine: '#2f7297' },
  header: {
    bg: '#0f2b40', ink: '#d9ecf8', muted: '#7fabc8', height: 52, border: '#245c7d',
    markBg: '#63d8ff', markRadius: '0px', markRotate: '45deg',
  },
  track: { bg: 'transparent', pad: 0, radius: 0, gap: 8, underline: '1px dashed #245c7d' },
  states: {
    tab: {
      on: { bg: 'transparent', fg: '#63d8ff', bd: '1px solid #63d8ff', ub: '1px solid #63d8ff' },
      off: { bg: 'transparent', fg: '#7fabc8', bd: '1px dashed #245c7d', ub: '1px dashed #245c7d' },
    },
    chip: {
      on: { bg: 'transparent', fg: '#63d8ff', bd: '#63d8ff' },
      off: { bg: 'transparent', fg: '#7fabc8', bd: '#245c7d' },
    },
  },
  sectionIndex: 'inline',
  chart: {
    bg: '#0b2233', grid: '#1b4763', dash: [3, 3], axis: '#245c7d', label: '#7fabc8',
    up: '#63d8ff', down: '#ff8f6b', style: 'outline', ma1: '#d9ecf8', ma2: '#ffd479', ema: '#8fd6a8', bb: '#3d7ba0',
    vol: '#164059', rsi: '#63d8ff', accent: '#63d8ff', accentInk: '#062130', surface2: '#143349', lw: 1.1,
  },
};

export const signalOrange: Theme = {
  id: 'signal-orange',
  name: '訊號橘工控',
  mode: 'dark',
  workspaceBackground: rasterWorkspaceBackground(
    signalOrangeBackgroundUrl,
    'linear-gradient(rgba(16,19,23,0.68), rgba(16,19,23,0.82))',
  ),
  color: {
    bg: '#101317', surface: '#191d23', surface2: '#21262d', cardBg: '#191d23',
    ink: '#e9e7e4', muted: '#969ca4', faint: '#6b7178', line: '#2a3037', line2: '#3a424b',
    accent: '#ff6a1f', accentInk: '#140800', accentWash: '#30221d', ok: '#4ec9a5', warn: '#ffb020', danger: '#ff5252',
  },
  font: {
    sans: `'Barlow Semi Condensed',${CJK.round},sans-serif`,
    mono: `'JetBrains Mono',${CJK.round},monospace`,
    size: 13, labelSize: 11, titleSize: 14,
    labelTransform: 'uppercase', labelTracking: '0.1em', labelWeight: 600,
    titleTransform: 'uppercase', titleTracking: '0.07em', titleWeight: 600,
  },
  shape: {
    radius: 6, radiusSm: 4, borderWidth: 1, borderStyle: 'solid', pad: 14, gap: 12,
    shadow: 'inset 0 1px 0 rgba(255,255,255,.04), 0 8px 20px -14px #000',
    fieldBg: '#14181d', fieldRadius: 4, btnRadius: 6, btnPad: '8px 14px', btnTransform: 'uppercase',
    btnWeight: 600, btnShadow: '0 1px 0 rgba(0,0,0,.5)', chipRadius: 4, tabRadius: 4, rowLine: '#21262d',
  },
  button: { primaryBg: '#ff6a1f', primaryInk: '#140800', ghostBg: '#21262d', ghostInk: '#e9e7e4', ghostLine: '#3a424b' },
  header: {
    bg: '#14181d', ink: '#e9e7e4', muted: '#969ca4', height: 54, border: '#2a3037',
    markBg: '#ff6a1f', markRadius: '2px', markRotate: '0deg',
  },
  track: { bg: '#14181d', pad: 3, radius: 6, gap: 3, underline: '0px solid transparent' },
  states: {
    tab: {
      on: { bg: '#ff6a1f', fg: '#140800', bd: '0px solid transparent', ub: '0px solid transparent' },
      off: { bg: 'transparent', fg: '#969ca4', bd: '0px solid transparent', ub: '0px solid transparent' },
    },
    chip: {
      on: { bg: '#ff6a1f', fg: '#140800', bd: '#ff6a1f' },
      off: { bg: '#21262d', fg: '#969ca4', bd: '#3a424b' },
    },
  },
  sectionIndex: 'none',
  chart: {
    bg: '#14181d', grid: '#21262d', dash: null, axis: '#2a3037', label: '#969ca4',
    up: '#4ec9a5', down: '#ff5252', style: 'filled', ma1: '#ff6a1f', ma2: '#7aa2ff', ema: '#c084fc', bb: '#4a525b',
    vol: '#2a3037', rsi: '#969ca4', accent: '#ff6a1f', accentInk: '#140800', surface2: '#21262d', lw: 1.5,
  },
};

export const broadsheet: Theme = {
  id: 'broadsheet',
  name: '早報紙本',
  mode: 'light',
  workspaceBackground: rasterWorkspaceBackground(
    paperBackgroundUrl,
    'linear-gradient(rgba(239,236,226,0.72), rgba(239,236,226,0.82))',
  ),
  color: {
    bg: '#efece2', surface: '#f8f6ef', surface2: '#e5e1d4', cardBg: '#f8f6ef',
    ink: '#1a1a17', muted: '#6b675c', faint: '#918c82', line: '#c9c4b6', line2: '#1a1a17',
    accent: '#a3231d', accentInk: '#f8f6ef', accentWash: '#f4e4df', ok: '#2a6b46', warn: '#8a6a1e', danger: '#a3231d',
  },
  font: {
    sans: `'Spectral',${CJK.songHK},serif`,
    mono: `'IBM Plex Mono',${CJK.songHK},monospace`,
    size: 13, labelSize: 10, titleSize: 17,
    labelTransform: 'uppercase', labelTracking: '0.1em', labelWeight: 600,
    titleTransform: 'none', titleTracking: '-0.01em', titleWeight: 700,
  },
  shape: {
    radius: 0, radiusSm: 0, borderWidth: 1, borderStyle: 'solid', pad: 16, gap: 14, shadow: 'none',
    fieldBg: '#fffefa', fieldRadius: 0, btnRadius: 0, btnPad: '7px 12px', btnTransform: 'none',
    btnWeight: 600, btnShadow: 'none', chipRadius: 0, tabRadius: 0, rowLine: '#ded9cb',
  },
  button: { primaryBg: '#1a1a17', primaryInk: '#f8f6ef', ghostBg: '#e5e1d4', ghostInk: '#1a1a17', ghostLine: '#c9c4b6' },
  header: {
    bg: '#f8f6ef', ink: '#1a1a17', muted: '#6b675c', height: 66, border: '#1a1a17',
    markBg: '#a3231d', markRadius: '0px', markRotate: '0deg',
  },
  track: { bg: 'transparent', pad: 0, radius: 0, gap: 0, underline: '1px solid #1a1a17' },
  states: {
    tab: {
      on: { bg: 'transparent', fg: '#1a1a17', bd: '0px solid transparent', ub: '3px solid #a3231d' },
      off: { bg: 'transparent', fg: '#6b675c', bd: '0px solid transparent', ub: '3px solid transparent' },
    },
    chip: {
      on: { bg: '#1a1a17', fg: '#f8f6ef', bd: '#1a1a17' },
      off: { bg: 'transparent', fg: '#6b675c', bd: '#c9c4b6' },
    },
  },
  sectionIndex: 'inline',
  chart: {
    bg: '#f8f6ef', grid: '#e0dbcd', dash: null, axis: '#c9c4b6', label: '#6b675c',
    up: '#1a1a17', down: '#a3231d', style: 'stick', ma1: '#1a1a17', ma2: '#a3231d', ema: '#4a6b8a', bb: '#c2bcaa',
    vol: '#d8d3c3', rsi: '#1a1a17', accent: '#a3231d', accentInk: '#f8f6ef', surface2: '#e5e1d4', lw: 1.2,
  },
};

export const brutalYellow: Theme = {
  id: 'brutal-yellow',
  name: '粗野派',
  mode: 'light',
  workspaceBackground: flatWorkspaceBackground,
  color: {
    bg: '#f2f0e6', surface: '#ffffff', surface2: '#ffe600', cardBg: '#ffffff',
    ink: '#000000', muted: '#4a4a45', faint: '#828279', line: '#000000', line2: '#000000',
    accent: '#2b32ff', accentInk: '#ffffff', accentWash: '#e6e6ff', ok: '#009e5c', warn: '#ff8a00', danger: '#ff2d2d',
  },
  font: {
    sans: `'Archivo',${CJK.heavy},sans-serif`,
    mono: `'Space Mono',${CJK.heavy},monospace`,
    size: 13, labelSize: 11, titleSize: 18,
    labelTransform: 'uppercase', labelTracking: '0.06em', labelWeight: 700,
    titleTransform: 'uppercase', titleTracking: '-0.02em', titleWeight: 900,
  },
  shape: {
    radius: 0, radiusSm: 0, borderWidth: 3, borderStyle: 'solid', pad: 16, gap: 16,
    shadow: '6px 6px 0 #000000', fieldBg: '#ffffff', fieldRadius: 0, btnRadius: 0,
    btnPad: '9px 14px', btnTransform: 'uppercase', btnWeight: 900, btnShadow: '4px 4px 0 #000000',
    chipRadius: 0, tabRadius: 0, rowLine: '#d8d6cc',
  },
  button: { primaryBg: '#2b32ff', primaryInk: '#ffffff', ghostBg: '#ffe600', ghostInk: '#000000', ghostLine: '#000000' },
  header: {
    bg: '#ffe600', ink: '#000000', muted: '#4a4a45', height: 68, border: '#000000',
    markBg: '#2b32ff', markRadius: '0px', markRotate: '0deg',
  },
  track: { bg: 'transparent', pad: 0, radius: 0, gap: 8, underline: '0px solid transparent' },
  states: {
    tab: {
      on: { bg: '#2b32ff', fg: '#ffffff', bd: '3px solid #000000', ub: '3px solid #000000' },
      off: { bg: '#ffffff', fg: '#000000', bd: '3px solid #000000', ub: '3px solid #000000' },
    },
    chip: {
      on: { bg: '#000000', fg: '#ffe600', bd: '#000000' },
      off: { bg: '#ffffff', fg: '#000000', bd: '#000000' },
    },
  },
  sectionIndex: 'inline',
  chart: {
    bg: '#ffffff', grid: '#e2e0d6', dash: null, axis: '#000000', label: '#000000',
    up: '#2b32ff', down: '#ff2d2d', style: 'block', ma1: '#000000', ma2: '#ff8a00', ema: '#8b00ff', bb: '#9a9a92',
    vol: '#000000', rsi: '#000000', accent: '#2b32ff', accentInk: '#ffffff', surface2: '#f2f0e6', lw: 3,
  },
};

export const frostGrey: Theme = {
  id: 'frost-grey',
  name: '霜灰極簡',
  mode: 'light',
  workspaceBackground: flatWorkspaceBackground,
  color: {
    bg: '#f5f7f8', surface: '#ffffff', surface2: '#eef1f3', cardBg: '#ffffff',
    ink: '#1f2933', muted: '#61707e', faint: '#8b959d', line: '#e3e8eb', line2: '#cfd7dd',
    accent: '#4a7c8c', accentInk: '#ffffff', accentWash: '#e9eff1', ok: '#3f8f6f', warn: '#a8843c', danger: '#b4574f',
  },
  font: {
    sans: `'Jost',${CJK.sans},sans-serif`,
    mono: `'IBM Plex Mono',${CJK.sans},monospace`,
    size: 13, labelSize: 10, titleSize: 15,
    labelTransform: 'uppercase', labelTracking: '0.12em', labelWeight: 500,
    titleTransform: 'none', titleTracking: '0.01em', titleWeight: 500,
  },
  shape: {
    radius: 4, radiusSm: 4, borderWidth: 1, borderStyle: 'solid', pad: 22, gap: 18, shadow: 'none',
    fieldBg: '#ffffff', fieldRadius: 4, btnRadius: 4, btnPad: '8px 14px', btnTransform: 'none',
    btnWeight: 500, btnShadow: 'none', chipRadius: 4, tabRadius: 4, rowLine: '#eef1f3',
  },
  button: { primaryBg: '#1f2933', primaryInk: '#ffffff', ghostBg: '#ffffff', ghostInk: '#1f2933', ghostLine: '#e3e8eb' },
  header: {
    bg: '#ffffff', ink: '#1f2933', muted: '#61707e', height: 64, border: '#e3e8eb',
    markBg: '#4a7c8c', markRadius: '999px', markRotate: '0deg',
  },
  track: { bg: 'transparent', pad: 0, radius: 0, gap: 0, underline: '1px solid #e3e8eb' },
  states: {
    tab: {
      on: { bg: 'transparent', fg: '#1f2933', bd: '0px solid transparent', ub: '1px solid #1f2933' },
      off: { bg: 'transparent', fg: '#aebac4', bd: '0px solid transparent', ub: '1px solid transparent' },
    },
    chip: {
      on: { bg: '#1f2933', fg: '#ffffff', bd: '#1f2933' },
      off: { bg: '#ffffff', fg: '#7b8794', bd: '#e3e8eb' },
    },
  },
  sectionIndex: 'none',
  chart: {
    bg: '#ffffff', grid: '#eef1f3', dash: null, axis: '#e3e8eb', label: '#61707e',
    up: '#4a7c8c', down: '#b4574f', style: 'line', ma1: '#9db4bf', ma2: '#c8a97a', ema: '#8a4f8f', bb: '#cbd4d9',
    vol: '#eef1f3', rsi: '#7b8794', accent: '#4a7c8c', accentInk: '#ffffff', surface2: '#eef1f3', lw: 1.6,
  },
};

export const auroraGlass: Theme = {
  id: 'aurora-glass',
  name: '霧面玻璃',
  mode: 'dark',
  workspaceBackground: rasterWorkspaceBackground(
    auroraBackgroundUrl,
    'linear-gradient(rgba(11,10,24,0.28), rgba(11,10,24,0.52))',
  ),
  color: {
    bg: '#0b0a18',
    surface: 'rgba(255,255,255,0.06)', surface2: 'rgba(255,255,255,0.10)', cardBg: 'rgba(255,255,255,0.055)',
    ink: '#eceafd', muted: '#a09cc9', faint: '#726ea0',
    line: 'rgba(255,255,255,0.14)', line2: 'rgba(255,255,255,0.26)',
    accent: '#a78bfa', accentInk: '#160f2e', accentWash: 'rgba(167,139,250,0.18)', ok: '#5eead4', warn: '#fbbf24', danger: '#fb7185',
  },
  font: {
    sans: `'Sora',${CJK.soft},sans-serif`,
    mono: `'JetBrains Mono',${CJK.soft},monospace`,
    size: 13, labelSize: 10, titleSize: 14,
    labelTransform: 'uppercase', labelTracking: '0.1em', labelWeight: 500,
    titleTransform: 'none', titleTracking: '0', titleWeight: 600,
  },
  shape: {
    radius: 16, radiusSm: 12, borderWidth: 1, borderStyle: 'solid', pad: 18, gap: 14,
    shadow: 'inset 0 1px 0 rgba(255,255,255,.14), 0 20px 44px -26px rgba(0,0,0,.95)',
    backdrop: 'blur(18px) saturate(1.35)',
    fieldBg: 'rgba(255,255,255,0.06)', fieldRadius: 10, btnRadius: 999, btnPad: '8px 16px',
    btnTransform: 'none', btnWeight: 600, btnShadow: '0 8px 18px -12px #000',
    chipRadius: 999, tabRadius: 999, rowLine: 'rgba(255,255,255,0.08)',
  },
  button: {
    primaryBg: '#a78bfa', primaryInk: '#160f2e',
    ghostBg: 'rgba(255,255,255,0.08)', ghostInk: '#eceafd', ghostLine: 'rgba(255,255,255,0.18)',
  },
  header: {
    bg: 'rgba(255,255,255,0.05)', ink: '#eceafd', muted: '#a09cc9', height: 60,
    border: 'rgba(255,255,255,0.12)', markBg: '#a78bfa', markRadius: '999px', markRotate: '0deg',
  },
  track: { bg: 'rgba(255,255,255,0.06)', pad: 4, radius: 999, gap: 4, underline: '0px solid transparent' },
  states: {
    tab: {
      on: { bg: 'rgba(255,255,255,0.16)', fg: '#eceafd', bd: '0px solid transparent', ub: '0px solid transparent' },
      off: { bg: 'transparent', fg: '#a09cc9', bd: '0px solid transparent', ub: '0px solid transparent' },
    },
    chip: {
      on: { bg: '#a78bfa', fg: '#160f2e', bd: '#a78bfa' },
      off: { bg: 'rgba(255,255,255,0.07)', fg: '#a09cc9', bd: 'rgba(255,255,255,0.16)' },
    },
  },
  sectionIndex: 'none',
  chart: {
    // Canvas needs an opaque colour; it sits inside the translucent card.
    bg: '#141230', grid: 'rgba(255,255,255,0.07)', dash: null,
    axis: 'rgba(255,255,255,0.14)', label: '#a09cc9',
    up: '#5eead4', down: '#fb7185', style: 'area', ma1: '#a78bfa', ma2: '#7dd3fc', ema: '#fbbf24', bb: 'rgba(255,255,255,0.24)',
    vol: 'rgba(255,255,255,0.09)', rsi: '#a78bfa',
    accent: '#a78bfa', accentInk: '#160f2e', surface2: '#2a2550', lw: 1.8,
  },
};

export const THEMES: Record<SkinId, Theme> = {
  'forge-paper': forgePaper,
  'midnight-tape': midnightTape,
  'swiss-forge': swissForge,
  'atelier-warm': atelierWarm,
  blueprint,
  'signal-orange': signalOrange,
  broadsheet,
  'brutal-yellow': brutalYellow,
  'frost-grey': frostGrey,
  'aurora-glass': auroraGlass,
};

export const SKIN_ORDER: SkinId[] = [
  'forge-paper', 'midnight-tape', 'swiss-forge', 'atelier-warm', 'blueprint', 'signal-orange',
  'broadsheet', 'brutal-yellow', 'frost-grey', 'aurora-glass',
];

export const DEFAULT_SKIN: SkinId = 'forge-paper';

export function isSkinId(id: string | null | undefined): id is SkinId {
  return typeof id === 'string' && Object.prototype.hasOwnProperty.call(THEMES, id);
}

export function getTheme(id: string | null | undefined): Theme {
  return isSkinId(id) ? THEMES[id] : THEMES[DEFAULT_SKIN];
}

/** Google Fonts families every skin needs, in one request. */
export const FONT_HREF =
  'https://fonts.googleapis.com/css2' +
  '?family=IBM+Plex+Sans:wght@400;500;600;700' +
  '&family=IBM+Plex+Mono:wght@400;500;600;700' +
  '&family=JetBrains+Mono:wght@400;500;700' +
  '&family=Space+Grotesk:wght@400;500;700' +
  '&family=Manrope:wght@400;500;600;700' +
  '&family=Barlow+Semi+Condensed:wght@400;500;600;700' +
  '&family=Noto+Sans+TC:wght@400;500;700' +
  '&family=Noto+Serif+TC:wght@400;600;700' +
  '&family=LXGW+WenKai+TC:wght@400;700' +
  '&family=Chiron+Hei+HK:wght@400;500;700' +
  '&family=Chiron+Sung+HK:wght@400;600;700' +
  '&family=Chiron+GoRound+TC:wght@400;500;700' +
  '&family=Spectral:wght@400;600;700' +
  '&family=Archivo:wght@500;700;900' +
  '&family=Space+Mono:wght@400;700' +
  '&family=Jost:wght@300;400;500' +
  '&family=Sora:wght@300;400;600' +
  '&family=Iansui' +
  '&family=Noto+Sans+HK:wght@300;700;900' +
  '&family=Huninn' +
  '&display=swap';
