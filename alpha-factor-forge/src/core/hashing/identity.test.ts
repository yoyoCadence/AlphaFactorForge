import { describe, expect, it } from 'vitest';
import fixture from './identity-v2.fixture.json';
import {
  DATASET_HASH_VERSION,
  STRATEGY_HASH_VERSION,
  datasetHash,
  normalizeDatasetCandles,
  strategyHash,
  strategyHashFromDefinitionJson,
  strategyHashSync,
} from '.';

describe('durable identity v2', () => {
  it('matches the committed strategy and dataset fixtures exactly', async () => {
    const strategy = await strategyHash(
      fixture.strategy.definition,
      fixture.strategy.execModel,
    );
    const dataset = await datasetHash(fixture.dataset.meta, fixture.dataset.candles);
    expect(strategy).toBe(fixture.strategy.expectedHash);
    expect(dataset).toBe(fixture.dataset.expectedHash);
  });

  it('recomputes a stored strategy definition and ignores object key order', async () => {
    const definitionJson = JSON.stringify(fixture.strategy.definition);
    const recomputed = await strategyHashFromDefinitionJson(definitionJson);
    const reordered = Object.fromEntries(Object.entries(fixture.strategy.definition).reverse());
    expect(recomputed).toBe(fixture.strategy.expectedHash);
    expect(await strategyHash(reordered, fixture.strategy.execModel)).toBe(recomputed);
    expect(recomputed).toMatch(new RegExp(`^${STRATEGY_HASH_VERSION}:[0-9a-f]{64}$`));
  });

  it('sorts dataset rows without mutating input and hashes every OHLCV value', async () => {
    const input = fixture.dataset.candles.map((candle) => ({ ...candle }));
    const original = JSON.stringify(input);
    const ordered = normalizeDatasetCandles(input);
    expect(ordered.map((candle) => candle.timestamp)).toEqual([
      1721001600000,
      1721005200000,
    ]);
    expect(JSON.stringify(input)).toBe(original);
    expect(await datasetHash(fixture.dataset.meta, [...input].reverse()))
      .toBe(fixture.dataset.expectedHash);
    expect(await datasetHash(fixture.dataset.meta, [
      { ...input[0], volume: input[0].volume + 1 },
      input[1],
    ])).not.toBe(fixture.dataset.expectedHash);
    expect(fixture.dataset.expectedHash)
      .toMatch(new RegExp(`^${DATASET_HASH_VERSION}:[0-9a-f]{64}$`));
  });

  it('fails closed for duplicate timestamps and non-finite values', async () => {
    const candle = fixture.dataset.candles[0];
    expect(() => normalizeDatasetCandles([candle, { ...candle }]))
      .toThrow(/duplicate candle timestamp/);
    await expect(datasetHash(fixture.dataset.meta, [{ ...candle, close: Infinity }]))
      .rejects.toThrow(/finite/);
  });

  it('labels the synchronous fallback as ephemeral', () => {
    expect(strategyHashSync(fixture.strategy.definition, fixture.strategy.execModel))
      .toMatch(/^ephemeral-fnv1a:[0-9a-f]{16}$/);
  });
});
