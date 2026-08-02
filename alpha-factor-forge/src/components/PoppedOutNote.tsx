// Inline stand-in shown where the chart / metrics normally sit while that
// section is popped out into a FloatingPanel (Slice 8a). Shared by ChartSection
// (chart pop-out) and BacktestPanel (metrics pop-out). Extracted verbatim.
import React from 'react';
import { makeStyles } from './panelStyles';
import { useTheme } from '../theme/ThemeProvider';

export function PoppedOutNote({ label, onClose }: { label: string; onClose: () => void }): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  return (
    // #f4f2ec (背景) 與 #cfccc4 (虛線框) 規格書 §4 未對照到 token，暫時沿用現值。
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '18px 12px', background: '#f4f2ec', border: '1px dashed #cfccc4', color: t.color.muted, fontSize: 12 }}>
      {label}已彈出放大檢視。
      <button style={{ ...S.btnGhost, padding: '2px 8px' }} onClick={onClose}>收合</button>
    </div>
  );
}
