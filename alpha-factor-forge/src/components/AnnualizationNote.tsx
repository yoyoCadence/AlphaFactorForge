import React from 'react';
import { annualizationYears, displayDays, shortAnnualizationDays, type AnnualizationColumn } from '../services/sampleSpan';
import { useTheme } from '../theme/ThemeProvider';

/** Shared beside every metrics view, including the native child window. */
export function AnnualizationNote({ columns, testId = 'annualization-note' }: {
  columns: readonly AnnualizationColumn[];
  testId?: string;
}): React.ReactElement | null {
  const t = useTheme();
  const short = columns.flatMap(({ label, sample }) => {
    const days = shortAnnualizationDays(sample);
    return days == null ? [] : [`${label}約 ${displayDays(days)} 天`];
  });
  const unknown = columns.filter(({ sample }) => annualizationYears(sample) == null).map(({ label }) => label);
  if (short.length === 0 && unknown.length === 0) return null;
  return (
    <p data-testid={testId} style={{ color: t.color.warn, fontSize: 11, margin: '8px 0 0' }}>
      {short.length > 0 && `${short.join('；')}，不足一年：CAGR、Sharpe、Sortino、Calmar 是年化後的數字，短樣本會被放大，僅供參考。`}
      {unknown.length > 0 && ` ${unknown.join('、')}的年化樣本根數未記錄，無法確認是否不足一年。`}
    </p>
  );
}
