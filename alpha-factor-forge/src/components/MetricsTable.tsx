import React from 'react';
import type { Metrics } from '../core/metrics';

export interface MetricsTableData {
  full: Metrics;
  inSample?: Metrics;
  outSample?: Metrics;
}

export interface MetricColumn {
  label: string;
  metrics: Metrics;
}

function pct(x: number): string {
  return `${(x * 100).toFixed(2)}%`;
}

function num(x: number): string {
  return Number.isFinite(x) ? x.toFixed(2) : '—';
}

const METRIC_ROWS: { label: string; fmt: (metrics: Metrics) => string }[] = [
  { label: '淨報酬', fmt: (metrics) => pct(metrics.netReturn) },
  { label: 'CAGR', fmt: (metrics) => pct(metrics.cagr) },
  { label: '最大回撤', fmt: (metrics) => pct(metrics.maxDrawdown) },
  { label: 'Sharpe', fmt: (metrics) => num(metrics.sharpe) },
  { label: 'Sortino', fmt: (metrics) => num(metrics.sortino) },
  { label: 'Calmar', fmt: (metrics) => num(metrics.calmar) },
  { label: '勝率', fmt: (metrics) => pct(metrics.winRate) },
  { label: '交易數', fmt: (metrics) => String(metrics.tradeCount) },
  { label: 'Profit Factor', fmt: (metrics) => (Number.isFinite(metrics.profitFactor) ? num(metrics.profitFactor) : '∞') },
  { label: '平均每筆', fmt: (metrics) => pct(metrics.avgTradeReturn) },
  { label: '曝險', fmt: (metrics) => pct(metrics.exposure) },
  { label: '換手', fmt: (metrics) => num(metrics.turnover) },
];

export function metricColumns(data: MetricsTableData): MetricColumn[] {
  if (data.inSample && data.outSample) {
    return [
      { label: '全期', metrics: data.full },
      { label: '樣本內', metrics: data.inSample },
      { label: '樣本外', metrics: data.outSample },
    ];
  }
  return [{ label: '', metrics: data.full }];
}

export function MetricsTable({ data, fontSize }: { data: MetricsTableData; fontSize: number }): React.ReactElement {
  const columns = metricColumns(data);
  return (
    <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: "'IBM Plex Mono', monospace", fontSize }}>
      {columns.length > 1 && (
        <thead>
          <tr style={{ borderBottom: '1px solid #d6d2c8' }}>
            <th />
            {columns.map((column) => (
              <th key={column.label} data-testid={`col-${column.label}`} style={{ padding: '4px', textAlign: 'right', fontSize: fontSize - 2, fontWeight: 600, color: '#8a8678' }}>
                {column.label}
              </th>
            ))}
          </tr>
        </thead>
      )}
      <tbody>
        {METRIC_ROWS.map((row) => (
          <tr key={row.label} style={{ borderBottom: '1px solid #efece5' }}>
            <td style={{ padding: '5px 4px', color: '#8a8678' }}>{row.label}</td>
            {columns.map((column) => (
              <td key={column.label} style={{ padding: '5px 4px', textAlign: 'right', fontWeight: 600 }}>
                {row.fmt(column.metrics)}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
