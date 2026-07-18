// METRIC-001: explicit non-finite encoding for JSON/DB boundaries.
//
// JSON.stringify silently turns Infinity/NaN into null, which destroys the
// distinction between "legitimately infinite" (e.g. Sortino with no downside)
// and "absent". Per the SCORE-001 handoff Resolution, every persistence
// boundary must state that status explicitly instead of relying on the
// implicit conversion. The status vocabulary matches the Resolution's
// `rawStatus` values so SCORE-001 can reuse it.

export type NonFiniteStatus = 'positive_infinity' | 'negative_infinity' | 'nan';

/** The explicit status of a non-finite number, or null when it is finite. */
export function nonFiniteStatus(x: number): NonFiniteStatus | null {
  if (Number.isFinite(x)) return null;
  if (Number.isNaN(x)) return 'nan';
  return x > 0 ? 'positive_infinity' : 'negative_infinity';
}
