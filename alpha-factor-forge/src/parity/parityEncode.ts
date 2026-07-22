// Shared expected-output encoding for the RS-CORE parity fixtures. ONE
// encoder (PR #65/#69 reviews: no second drifting codec): finite metric
// leaves stay numbers, legitimate non-finite values become their METRIC-001
// status string, and monthly returns must be finite by construction.

import type { Metrics } from '../core/metrics';
import type { NonFiniteStatus } from '../services/nonFinite';
import { nonFiniteStatus } from '../services/nonFinite';

/** Finite number, or the METRIC-001 status of a legitimate non-finite value. */
export type MetricLeaf = number | NonFiniteStatus;

export function encodeMetricsForParity(
  metrics: Metrics,
): Record<string, MetricLeaf | Record<string, number>> {
  const encoded: Record<string, MetricLeaf | Record<string, number>> = {};
  for (const [key, value] of Object.entries(metrics)) {
    if (typeof value === 'number') {
      encoded[key] = nonFiniteStatus(value) ?? value;
    } else {
      for (const monthly of Object.values(value as Record<string, number>)) {
        if (!Number.isFinite(monthly)) throw new Error('monthly returns must stay finite');
      }
      encoded[key] = { ...(value as Record<string, number>) };
    }
  }
  return encoded;
}
