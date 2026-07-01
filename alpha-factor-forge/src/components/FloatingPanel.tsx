// Slice 8a — reusable floating, draggable, resizable pop-out panel.
//
// An in-app enlarge view: a `position: fixed` overlay with a drag title bar and
// a bottom-right resize handle. Deliberately NON-modal — there is no backdrop,
// so the rest of the app (the left-column strategy/exec/dataset controls) stays
// fully interactive while the panel is open, and edits reflow the enlarged
// content live because it is the same React tree.
//
// `children` is a render prop receiving the current inner content size, so a
// canvas child (the chart) can fill the panel; content that doesn't care about
// size can just ignore the argument. Close via the ✕ button or Escape.
//
// The real-OS-window variant (drag to another monitor) is the future Slice 8b.

import React, { useEffect, useRef, useState } from 'react';

const TITLE_H = 30;
const PAD = 8;
const MIN_W = 280;
const MIN_H = 180;

export function FloatingPanel({
  title,
  testId,
  onClose,
  initial,
  children,
}: {
  title: string;
  /** Test/aria hook: the panel gets `data-testid={testId}`, close = `${testId}-close`. */
  testId?: string;
  onClose: () => void;
  initial?: { x?: number; y?: number; w?: number; h?: number };
  children: (size: { w: number; h: number }) => React.ReactNode;
}): React.ReactElement {
  const [pos, setPos] = useState({ x: initial?.x ?? 140, y: initial?.y ?? 90 });
  const [size, setSize] = useState({ w: initial?.w ?? 760, h: initial?.h ?? 500 });
  // Active drag/resize gesture; null when idle. Held in a ref so the window
  // listeners read the latest start values without re-subscribing.
  const gesture = useRef<
    | { mode: 'move' | 'resize'; sx: number; sy: number; px: number; py: number; pw: number; ph: number }
    | null
  >(null);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      const g = gesture.current;
      if (!g) return;
      const dx = e.clientX - g.sx;
      const dy = e.clientY - g.sy;
      if (g.mode === 'move') {
        // keep the title bar on-screen so the panel can always be grabbed again
        const x = Math.max(0, Math.min(window.innerWidth - 80, g.px + dx));
        const y = Math.max(0, Math.min(window.innerHeight - TITLE_H, g.py + dy));
        setPos({ x, y });
      } else {
        setSize({ w: Math.max(MIN_W, g.pw + dx), h: Math.max(MIN_H, g.ph + dy) });
      }
    };
    const onUp = () => {
      gesture.current = null;
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  const start = (mode: 'move' | 'resize') => (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    gesture.current = { mode, sx: e.clientX, sy: e.clientY, px: pos.x, py: pos.y, pw: size.w, ph: size.h };
  };

  const contentW = size.w - PAD * 2;
  const contentH = size.h - TITLE_H - PAD * 2;

  return (
    <div
      role="dialog"
      aria-modal={false}
      aria-label={title}
      data-testid={testId}
      style={{
        position: 'fixed',
        left: pos.x,
        top: pos.y,
        width: size.w,
        height: size.h,
        zIndex: 1000,
        background: '#fff',
        border: '1px solid #16150f',
        boxShadow: '0 8px 30px rgba(0,0,0,0.28)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div
        onMouseDown={start('move')}
        style={{
          height: TITLE_H,
          flex: '0 0 auto',
          cursor: 'move',
          background: '#16150f',
          color: '#fff',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 8px',
          fontFamily: "'IBM Plex Mono', monospace",
          fontSize: 12,
          fontWeight: 700,
          userSelect: 'none',
        }}
      >
        <span>⤢ {title}</span>
        <button
          type="button"
          data-testid={testId ? `${testId}-close` : undefined}
          aria-label="關閉放大"
          onClick={onClose}
          onMouseDown={(e) => e.stopPropagation()}
          style={{ border: 'none', background: 'transparent', color: '#fff', cursor: 'pointer', fontSize: 14, lineHeight: 1, padding: 2 }}
        >
          ✕
        </button>
      </div>
      <div style={{ flex: 1, padding: PAD, overflow: 'auto', minHeight: 0 }}>{children({ w: contentW, h: contentH })}</div>
      <div
        onMouseDown={start('resize')}
        title="拖曳縮放"
        style={{
          position: 'absolute',
          right: 0,
          bottom: 0,
          width: 16,
          height: 16,
          cursor: 'nwse-resize',
          background: 'linear-gradient(135deg, transparent 50%, #b8b3a6 50%)',
        }}
      />
    </div>
  );
}
