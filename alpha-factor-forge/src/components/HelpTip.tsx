// Slice 5c — reusable clickable "?" help marker.
//
// A small circular "?" button that toggles a short explanation popover. UI-only
// and self-contained (owns its open state; no logic/data dependencies), so it
// can be dropped next to any BacktestPanel section header or action button.
//
// Behaviour: click the "?" to toggle; Escape or a click outside closes it. The
// button click stops propagation so a HelpTip placed inside a <label> (e.g. the
// Holdout row) can't also toggle that label's control.

import React, { useEffect, useId, useRef, useState } from 'react';

export function HelpTip({
  id,
  label,
  text,
  align = 'left',
}: {
  /** Stable ascii key for the test hook (`help-<id>`) + aria wiring. */
  id: string;
  /** Short human name for the accessible label (e.g. 資料集). */
  label: string;
  /** The explanation shown in the popover. */
  text: string;
  /** Which edge of the "?" the popover is anchored to (avoids overflow near a
   *  container's right edge). Defaults to opening rightward. */
  align?: 'left' | 'right';
}): React.ReactElement {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);
  const popId = useId();

  // While open, close on an outside click or Escape. Listeners are only attached
  // when open, so a page of HelpTips isn't all listening at once.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <span ref={ref} style={{ position: 'relative', display: 'inline-flex', lineHeight: 0 }}>
      <button
        type="button"
        data-testid={`help-${id}`}
        aria-label={`${label}說明`}
        aria-expanded={open}
        aria-describedby={open ? popId : undefined}
        onClick={(e) => {
          // Don't let the click reach an enclosing <label>/row handler.
          e.stopPropagation();
          e.preventDefault();
          setOpen((o) => !o);
        }}
        style={{
          width: 15,
          height: 15,
          borderRadius: '50%',
          border: '1px solid #b8b3a6',
          background: open ? '#16150f' : '#efece5',
          color: open ? '#fff' : '#8a8678',
          fontFamily: "'IBM Plex Mono', monospace",
          fontSize: 10,
          fontWeight: 700,
          lineHeight: '13px',
          textAlign: 'center',
          padding: 0,
          cursor: 'pointer',
        }}
      >
        ?
      </button>
      {open && (
        <span
          id={popId}
          role="tooltip"
          // Clicks inside the popover shouldn't bubble to an enclosing row.
          onClick={(e) => e.stopPropagation()}
          style={{
            position: 'absolute',
            top: 20,
            [align]: 0,
            zIndex: 20,
            width: 240,
            background: '#16150f',
            color: '#f4f1e9',
            fontFamily: "'IBM Plex Mono', monospace",
            fontSize: 11,
            fontWeight: 400,
            lineHeight: 1.5,
            letterSpacing: 0,
            textAlign: 'left',
            whiteSpace: 'normal',
            padding: '8px 10px',
            border: '1px solid #3c3a30',
            boxShadow: '0 4px 14px rgba(0,0,0,0.25)',
          }}
        >
          {text}
        </span>
      )}
    </span>
  );
}
