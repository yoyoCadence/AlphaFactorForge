// Numeric, editable keys of ParamsStrategy that the panel form and the chart
// quick row bind NumberInputs to (and that the sweep applied-highlight tracks).
// Shared here so BacktestPanel and ChartSection agree on the type without a
// circular import.
export type NumKey =
  | 'fastMA' | 'slowMA' | 'emaPeriod' | 'rsiPeriod' | 'rsiBuy' | 'rsiSell'
  | 'macdFast' | 'macdSlow' | 'macdSignal' | 'bbPeriod' | 'bbMult'
  | 'feePct' | 'slipPct' | 'sizePct' | 'slPct' | 'tpPct';
