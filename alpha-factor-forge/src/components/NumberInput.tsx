// Numeric input that allows clearing / partial edits. Keeps a draft string
// while focused (so backspace doesn't snap to 0 or a clamp), propagates the
// number live only when the draft is a valid number, and normalises/clamps on
// blur. min/max clamp on blur only — not while typing.
//
// Extracted verbatim from BacktestPanel so the panel and the SweepSection's
// AxisEditor can share it (move-only; no behaviour change).
import React, { useEffect, useState } from 'react';

/** The value a blur commits: the draft when it parses to a finite number,
 *  otherwise the current value, clamped to [min, max]. Pure. */
export function blurCommitValue(draft: string, value: number, min?: number, max?: number): number {
  const parsed = parseFloat(draft);
  let committed = Number.isFinite(parsed) ? parsed : value;
  if (min != null) committed = Math.max(min, committed);
  if (max != null) committed = Math.min(max, committed);
  return committed;
}

export function NumberInput({
  value,
  onChange,
  min,
  max,
  step,
  style,
}: {
  value: number;
  onChange: (n: number) => void;
  min?: number;
  max?: number;
  /** Spinner/keyboard granularity hint only (STRATEGY-VALIDATION-001 secondary
   *  protection). It is NOT enforcement: a typed or pasted value still reaches
   *  onChange unchanged, which is why the runtime validator exists. */
  step?: number;
  style?: React.CSSProperties;
}): React.ReactElement {
  const [draft, setDraft] = useState(String(value));

  // Re-sync when the value changes externally, but don't clobber an in-progress
  // edit that already parses to the same number (e.g. "5." while typing "5.5").
  useEffect(() => {
    if (parseFloat(draft) !== value) setDraft(String(value));
  }, [value]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <input
      type="number"
      min={min}
      max={max}
      step={step}
      value={draft}
      onChange={(e) => {
        const raw = e.target.value;
        setDraft(raw);
        const n = parseFloat(raw);
        if (raw !== '' && Number.isFinite(n)) onChange(n); // live, unclamped; empty/partial stays in the field
      }}
      onBlur={() => {
        const v = blurCommitValue(draft, value, min, max);
        // BUG-SWEEP-BLUR-001: leaving the field is an edit only when it changes
        // the value — in practice a clamp, since the live path has already sent
        // every parseable draft. Reporting an unchanged value made every
        // consumer treat focus-then-blur as an edit: the sweep cleared its
        // finished heatmap and the strategy form dropped its applied-from-sweep
        // marker.
        if (!Object.is(v, value)) onChange(v);
        setDraft(String(v));
      }}
      style={style}
    />
  );
}
