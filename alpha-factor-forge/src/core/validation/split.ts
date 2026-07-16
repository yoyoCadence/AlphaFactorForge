// Phase B validation foundation: deterministic, time-ordered bar ranges.
// Pure TypeScript only — no React, DOM, IO, persistence, or ranking behavior.

export const V1_VALIDATION_RATIOS = {
  train: 0.6,
  validation: 0.2,
  test: 0.2,
} as const;

export interface InclusiveBarRange {
  /** Zero-based inclusive first bar, compatible with BacktestConfig.from. */
  readonly from: number;
  /** Zero-based inclusive last bar, compatible with BacktestConfig.to. */
  readonly to: number;
  readonly count: number;
}

export interface ValidationSplitPlan {
  readonly totalBars: number;
  /** Bars available to Train/Validation/Test after both embargo gaps. */
  readonly usableBars: number;
  /** Size of each boundary gap; the caller owns how this is derived. */
  readonly embargoBars: number;
  readonly train: InclusiveBarRange;
  readonly trainValidationEmbargo: InclusiveBarRange | null;
  readonly validation: InclusiveBarRange;
  readonly validationTestEmbargo: InclusiveBarRange | null;
  /** Always the final evaluated range. It must not drive v1 ranking or prompts. */
  readonly test: InclusiveBarRange;
}

interface Allocation {
  train: number;
  validation: number;
  test: number;
}

const MIN_USABLE_BARS = 5;
const V1_RATIO_WEIGHTS = [3, 1, 1] as const;
const V1_RATIO_DENOMINATOR = 5;

function assertNonNegativeSafeInteger(value: number, name: string): void {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError(`${name} must be a non-negative safe integer`);
  }
}

/**
 * Allocate the usable bars with the largest-remainder method. Equal
 * fractional remainders are resolved in Train -> Validation -> Test order.
 */
function allocateUsableBars(usableBars: number): Allocation {
  // Split first so we never multiply a large safe integer by a floating-point
  // ratio (or by 3 directly). Every intermediate remains an exact safe integer.
  const quotient = Math.floor(usableBars / V1_RATIO_DENOMINATOR);
  const tail = usableBars % V1_RATIO_DENOMINATOR;
  const counts = V1_RATIO_WEIGHTS.map(
    (weight) => weight * quotient + Math.floor((weight * tail) / V1_RATIO_DENOMINATOR),
  );
  const remainderOrder = V1_RATIO_WEIGHTS
    .map((weight, index) => ({
      index,
      numerator: (weight * tail) % V1_RATIO_DENOMINATOR,
    }))
    .sort((a, b) => b.numerator - a.numerator || a.index - b.index);

  let remaining = usableBars - counts.reduce((sum, count) => sum + count, 0);
  for (let i = 0; remaining > 0; i++, remaining--) {
    counts[remainderOrder[i].index] += 1;
  }

  const [train, validation, test] = counts;
  if (train < 1 || validation < 1 || test < 1) {
    throw new RangeError(
      `at least ${MIN_USABLE_BARS} usable bars are required for non-empty 60/20/20 segments`,
    );
  }
  return { train, validation, test };
}

function rangeFrom(cursor: number, count: number): InclusiveBarRange {
  return { from: cursor, to: cursor + count - 1, count };
}

/**
 * Plan the v1 Train/Validation/Test split over bars sorted oldest to newest.
 *
 * Contract:
 * 1. Exclude two equal embargo gaps from the total bar count.
 * 2. Allocate the remaining bars 60/20/20 with deterministic largest-remainder
 *    rounding (ties favor Train, then Validation, then Test).
 * 3. Return inclusive, ascending ranges that account for every input bar once.
 *
 * Invalid or insufficient input throws instead of silently weakening an
 * embargo or returning an empty evaluation segment.
 */
export function planValidationSplit(totalBars: number, embargoBars: number): ValidationSplitPlan {
  assertNonNegativeSafeInteger(totalBars, 'totalBars');
  assertNonNegativeSafeInteger(embargoBars, 'embargoBars');

  const usableBars = totalBars - embargoBars * 2;
  if (usableBars < MIN_USABLE_BARS) {
    throw new RangeError(
      `at least ${MIN_USABLE_BARS} usable bars are required after two embargo gaps`,
    );
  }

  const allocation = allocateUsableBars(usableBars);
  let cursor = 0;

  const train = rangeFrom(cursor, allocation.train);
  cursor = train.to + 1;

  const trainValidationEmbargo = embargoBars > 0 ? rangeFrom(cursor, embargoBars) : null;
  cursor += embargoBars;

  const validation = rangeFrom(cursor, allocation.validation);
  cursor = validation.to + 1;

  const validationTestEmbargo = embargoBars > 0 ? rangeFrom(cursor, embargoBars) : null;
  cursor += embargoBars;

  const test = rangeFrom(cursor, allocation.test);

  return {
    totalBars,
    usableBars,
    embargoBars,
    train,
    trainValidationEmbargo,
    validation,
    validationTestEmbargo,
    test,
  };
}
