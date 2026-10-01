// P12e-2 — independent reference for `research-alpha-allocation-v1`, written
// from docs/research-alpha-allocation-v1.md rather than ported from the Rust
// module. It produces the `expected` blocks of
// fixtures/rs-core/research-alpha-allocation-v1.json; the Rust tests must
// reproduce that fixture. Test support only: no product path imports it.

import { canonicalBytes, sha256BytesHex } from '../core/hashing';

export const ALPHA_ALLOCATION_VERSION = 'research-alpha-allocation-v1';
const ALPHA_PPM_MAX = 999_999;

export interface AlphaAllocationDeclaration {
  contractVersion: string;
  rule: string;
  scope: string;
  totalAlphaPpm: number;
  schedule: number[];
}

export interface AlphaAllocationFixtureCase {
  id: string;
  declaration: AlphaAllocationDeclaration;
  reserved: number[];
}

export interface AlphaAllocationReport {
  contractVersion: string;
  rule: string;
  scope: string;
  allocationId: string;
  status: 'ELIGIBLE' | 'NOT_ELIGIBLE';
  reasons: string[];
  totalAlphaPpm: number;
  scheduledConfirmations: number;
  reservedConfirmations: number;
  spentAlphaPpm: number;
  confirmationNumber: number | null;
  alphaPpm: number | null;
  remainingConfirmations: number;
  remainingScheduledAlphaPpm: number;
  unscheduledAlphaPpm: number;
}

function isAlpha(value: unknown): value is number {
  return Number.isInteger(value) && (value as number) >= 1 && (value as number) <= ALPHA_PPM_MAX;
}

function sum(values: readonly number[]): number {
  return values.reduce((total, value) => total + value, 0);
}

function assertDeclared(declaration: AlphaAllocationDeclaration): void {
  if (
    declaration.contractVersion !== ALPHA_ALLOCATION_VERSION ||
    declaration.rule !== 'declared-schedule' ||
    declaration.scope !== 'trial-family' ||
    !isAlpha(declaration.totalAlphaPpm) ||
    !Array.isArray(declaration.schedule) ||
    declaration.schedule.length === 0 ||
    !declaration.schedule.every(isAlpha) ||
    sum(declaration.schedule) > declaration.totalAlphaPpm
  ) {
    throw new Error('declaration is outside research-alpha-allocation-v1');
  }
}

/** SHA-256 over the contract version, a zero byte and the canonical document. */
export async function referenceAllocationId(
  declaration: AlphaAllocationDeclaration,
): Promise<string> {
  assertDeclared(declaration);
  const version = new TextEncoder().encode(ALPHA_ALLOCATION_VERSION);
  const document = canonicalBytes({
    contractVersion: declaration.contractVersion,
    rule: declaration.rule,
    scope: declaration.scope,
    totalAlphaPpm: declaration.totalAlphaPpm,
    schedule: declaration.schedule,
  });
  const bytes = new Uint8Array(version.length + 1 + document.length);
  bytes.set(version, 0);
  bytes.set(document, version.length + 1);
  return sha256BytesHex(bytes);
}

/** The report for the family's next confirmation; throws on a refused input. */
export async function referenceAllocation(
  testCase: Pick<AlphaAllocationFixtureCase, 'declaration' | 'reserved'>,
): Promise<AlphaAllocationReport> {
  const { declaration, reserved } = testCase;
  assertDeclared(declaration);
  const { schedule, totalAlphaPpm } = declaration;
  if (
    reserved.length > schedule.length ||
    reserved.some((alpha, index) => alpha !== schedule[index])
  ) {
    throw new Error('reserved history contradicts the schedule');
  }
  const used = reserved.length;
  const next = used < schedule.length ? schedule[used] : null;
  return {
    contractVersion: declaration.contractVersion,
    rule: declaration.rule,
    scope: declaration.scope,
    allocationId: await referenceAllocationId(declaration),
    status: next === null ? 'NOT_ELIGIBLE' : 'ELIGIBLE',
    reasons: next === null ? ['alpha_budget_exhausted'] : [],
    totalAlphaPpm,
    scheduledConfirmations: schedule.length,
    reservedConfirmations: used,
    spentAlphaPpm: sum(schedule.slice(0, used)),
    confirmationNumber: next === null ? null : used + 1,
    alphaPpm: next,
    remainingConfirmations: next === null ? 0 : schedule.length - used - 1,
    remainingScheduledAlphaPpm: sum(schedule.slice(used + 1)),
    unscheduledAlphaPpm: totalAlphaPpm - sum(schedule),
  };
}

/** Equal shares, floored; throws when a share would be below 1 ppm. */
export function referenceEqualSchedule(totalAlphaPpm: number, confirmations: number): number[] {
  if (
    !isAlpha(totalAlphaPpm) ||
    !Number.isInteger(confirmations) ||
    confirmations < 1 ||
    confirmations > totalAlphaPpm
  ) {
    throw new Error('equal split is outside research-alpha-allocation-v1');
  }
  const share = Math.floor(totalAlphaPpm / confirmations);
  return Array.from({ length: confirmations }, () => share);
}
