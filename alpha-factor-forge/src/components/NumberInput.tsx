// Numeric input that allows clearing / partial edits. Keeps a draft string
// while focused (so backspace doesn't snap to 0 or a clamp), propagates the
// number live only when the draft is a valid number, and normalises/clamps on
// blur. min/max clamp on blur only — not while typing.
//
// Extracted verbatim from BacktestPanel so the panel and the SweepSection's
// AxisEditor can share it (move-only; no behaviour change).
import React, { useEffect, useState } from 'react';

export function NumberInput({
  value,
  onChange,
  min,
  max,
  style,
}: {
  value: number;
  onChange: (n: number) => void;
  min?: number;
  max?: number;
  style?: React.CSSProperties;
}): React.ReactElement {
  const [draft, setDraft] = useState(String(value));

  // Re-sync when the value changes externally, but don't clobber an in-progress
  // edit that already parses to the same number (e.g. "5." while typing "5.5").
  useEffect(() => {
    if (parseFloat(draft) !== value) setDraft(String(value));
  }, [value]); // eslint-disable-line react-hooks/exhaustive-deps

  const clamp = (n: number) => {
    let v = n;
    if (min != null) v = Math.max(min, v);
    if (max != null) v = Math.min(max, v);
    return v;
  };

  return (
    <input
      type="number"
      value={draft}
      onChange={(e) => {
        const raw = e.target.value;
        setDraft(raw);
        const n = parseFloat(raw);
        if (raw !== '' && Number.isFinite(n)) onChange(n); // live, unclamped; empty/partial stays in the field
      }}
      onBlur={() => {
        const n = parseFloat(draft);
        const v = clamp(Number.isFinite(n) ? n : value);
        onChange(v);
        setDraft(String(v));
      }}
      style={style}
    />
  );
}
