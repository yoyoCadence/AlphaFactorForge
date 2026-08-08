// Skin picker — the switcher UI itself.
//
// Drop-in target: alpha-factor-forge/src/components/SkinPicker.tsx
// Mount it in main.tsx's <header>, right of the status text.
//
// Compact by design: a labelled <select> is the smallest thing that fits the
// existing 50px header and needs no popover/focus-trap work. The swatch strip
// gives the visual cue that these are whole looks, not a colour setting.

import React from 'react';
import { SKIN_ORDER, THEMES, type SkinId } from '../theme/theme';
import { useSkin, useTheme } from '../theme/ThemeProvider';

export function SkinPicker(): React.ReactElement {
  const t = useTheme();
  const { skin, setSkin } = useSkin();

  return (
    <label
      className="skin-picker"
      data-testid="skin-picker"
      style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' }}
    >
      <span
        className="skin-picker-label"
        style={{
          fontFamily: t.font.mono,
          fontSize: t.font.labelSize,
          letterSpacing: t.font.labelTracking,
          textTransform: t.font.labelTransform,
          color: t.header.muted,
        }}
      >
        皮膚
      </span>

      <span className="skin-picker-swatches" aria-hidden="true" style={{ display: 'flex', gap: 3 }}>
        {[t.color.bg, t.color.accent, t.chart.up].map((hex, index) => (
          <span
            key={`${index}-${hex}`}
            style={{
              width: 9,
              height: 9,
              background: hex,
              borderRadius: t.shape.chipRadius > 8 ? '50%' : 0,
              outline: `1px solid ${t.color.line2}`,
              outlineOffset: -1,
            }}
          />
        ))}
      </span>

      <select
        aria-label='皮膚'
        value={skin}
        onChange={(e) => setSkin(e.target.value as SkinId)}
        style={{
          padding: '4px 6px',
          background: t.shape.fieldBg,
          color: t.color.ink,
          border: `${t.shape.borderWidth}px solid ${t.color.line}`,
          borderRadius: t.shape.fieldRadius,
          fontFamily: t.font.mono,
          fontSize: 11,
        }}
      >
        {SKIN_ORDER.map((id, i) => (
          <option key={id} value={id}>
            {String(i + 1).padStart(2, '0')} {THEMES[id].name}
          </option>
        ))}
      </select>
    </label>
  );
}
