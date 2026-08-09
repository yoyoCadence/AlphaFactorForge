import { describe, expect, it } from 'vitest';
import fixture from '../core/hashing/identity-v2.fixture.json';
import { defaultStrategy } from '../services/strategy';
import { buildStrategyDef } from '../services/strategyRecord';
import { prepareDatasetImport } from './dbClient';
import { makeMockClient } from './mockClient';

const importInput = () => ({
  ...fixture.dataset.meta,
  source: 'identity-test',
  candles: fixture.dataset.candles.map((candle) => ({ ...candle })),
});

describe('durable identity client boundary', () => {
  it('prepares sorted content metadata and the committed v2 dataset hash', async () => {
    const prepared = await prepareDatasetImport(importInput());
    expect(prepared.dataset.dataset_hash).toBe(fixture.dataset.expectedHash);
    expect(prepared.dataset.start_time).toBe(1721001600000);
    expect(prepared.dataset.end_time).toBe(1721005200000);
    expect(prepared.dataset.candle_count).toBe(2);
    expect(prepared.candles.map((candle) => candle.timestamp)).toEqual([
      1721001600000,
      1721005200000,
    ]);
  });

  it('keeps mock imports idempotent and rejects a forged boundary payload', async () => {
    const client = makeMockClient();
    const first = await client.importDataset(importInput());
    const second = await client.importDataset(importInput());
    expect(second).toBe(first);
    expect(await client.db.getDatasets()).toHaveLength(1);

    const prepared = await prepareDatasetImport(importInput());
    await expect(client.db.importCandles({
      ...prepared.dataset,
      dataset_hash: 'dataset-content-v2:forged',
    }, prepared.candles)).rejects.toThrow(/identity/);
    expect(await client.db.getDatasets()).toHaveLength(1);
  });

  /**
   * DATA-QUALITY-001 step 6 (TypeScript counterpart): a rejected
   * prepareDatasetImport must leave the mock store untouched, and must throw
   * before any db.importCandles boundary call is reached.
   *
   * Rule 1 (`timestamp_not_integer`) is absent for the same structural reason as
   * on the Rust side: `normalizeDatasetCandles` runs its own
   * `Number.isSafeInteger` check first, so the quality gate can never classify
   * it here. Rule 3 is unreachable by construction and is covered by the direct
   * predicate test in src/core/market-data/quality.test.ts.
   */
  it('rejects every reachable market-data rule at import and writes nothing', async () => {
    // The reported index is the position in NORMALIZED (timestamp-sorted)
    // order, which is why the out-of-range case reports 0 rather than 1.
    const cases: [string, Record<string, number>, number][] = [
      // Epoch SECONDS for 2024-01-01, silently read as milliseconds.
      ['timestamp_out_of_range', { timestamp: 1_704_067_200 }, 0],
      ['price_not_positive', { low: 0 }, 1],
      ['volume_negative', { volume: -1 }, 1],
      ['high_below_low', { open: 100, high: 99, low: 100, close: 100 }, 1],
      ['ohlc_out_of_range', { open: 1_000_000 }, 1],
    ];

    for (const [rule, patch, index] of cases) {
      const client = makeMockClient();
      await client.importDataset(importInput());
      const before = await client.db.getDatasets();
      expect(before, rule).toHaveLength(1);

      const mutated = importInput();
      // Mutate the LATEST candle, so it normalizes to index 1 unless the
      // mutation is the timestamp itself (which re-sorts it to index 0).
      const target = mutated.candles.reduce(
        (latest, candle, at) => (candle.timestamp > mutated.candles[latest].timestamp ? at : latest),
        0,
      );
      mutated.candles[target] = { ...mutated.candles[target], ...patch };
      // The rejection is the quality gate's, naming the rule and the failing
      // candle — not an identity mismatch.
      await expect(prepareDatasetImport(mutated), rule).rejects.toThrow(
        new RegExp(`^market data rejected at candle ${index} \\(timestamp -?\\d+\\): ${rule}$`),
      );
      await expect(client.importDataset(mutated), rule).rejects.toThrow(rule);
      expect(await client.db.getDatasets(), rule).toEqual(before);
    }
  });

  it('accepts verified strategies and rejects legacy or forged hashes', async () => {
    const client = makeMockClient();
    const definition = await buildStrategyDef(defaultStrategy(), 'v2 strategy');
    await expect(client.db.saveStrategy(definition)).resolves.toBeGreaterThan(0);
    await expect(client.db.saveStrategy({
      ...definition,
      strategy_hash: 'legacy-unversioned',
    })).rejects.toThrow(/identity/);
  });
});
