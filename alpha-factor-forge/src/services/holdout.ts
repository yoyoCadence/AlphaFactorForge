// Shared holdout split math. In-sample is [0, i-1], out-of-sample is [i, n-1]
// (the last `holdoutPct`% of bars). Clamped so both sides stay non-empty. Used
// by the backtest run() and the parameter sweep so they optimise/validate on the
// exact same boundary (BUG-001). Pure — no IO/state.
export function holdoutSplitIndex(n: number, holdoutPct: number): number {
  return Math.max(1, Math.min(n - 1, Math.floor(n * (1 - holdoutPct / 100))));
}
