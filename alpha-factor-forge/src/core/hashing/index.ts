// Durable identity contracts shared with the Rust backend. Persisted hashes
// are versioned SHA-256 values over explicit binary encodings; they never
// silently change algorithm when a runtime capability is missing.

export const STRATEGY_HASH_VERSION = 'strategy-v2';
export const DATASET_HASH_VERSION = 'dataset-content-v2';
export const DATASET_FIELD_MAPPING_VERSION =
  'ohlcv(timestamp:i64-ms,open:f64,high:f64,low:f64,close:f64,volume:f64)-v1';

const textEncoder = new TextEncoder();

/** Canonical JSON for readable snapshots. Durable identities use canonicalBytes. */
export function canonicalize(value: unknown): string {
  return JSON.stringify(sortDeep(value));
}

function sortDeep(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortDeep);
  if (value && typeof value === 'object') {
    const out: Record<string, unknown> = {};
    for (const key of Object.keys(value as Record<string, unknown>).sort(compareUtf8)) {
      out[key] = sortDeep((value as Record<string, unknown>)[key]);
    }
    return out;
  }
  return value;
}

function compareUtf8(a: string, b: string): number {
  const left = textEncoder.encode(a);
  const right = textEncoder.encode(b);
  const length = Math.min(left.length, right.length);
  for (let i = 0; i < length; i++) {
    if (left[i] !== right[i]) return left[i] - right[i];
  }
  return left.length - right.length;
}

class ByteWriter {
  private bytes = new Uint8Array(256);
  private length = 0;

  private reserve(extra: number): void {
    const required = this.length + extra;
    if (required <= this.bytes.length) return;
    let capacity = this.bytes.length;
    while (capacity < required) capacity *= 2;
    const grown = new Uint8Array(capacity);
    grown.set(this.bytes);
    this.bytes = grown;
  }

  byte(value: number): void {
    this.reserve(1);
    this.bytes[this.length++] = value & 0xff;
  }

  raw(value: Uint8Array): void {
    this.reserve(value.length);
    this.bytes.set(value, this.length);
    this.length += value.length;
  }

  u32(value: number): void {
    if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
      throw new Error(`identity length is out of u32 range: ${value}`);
    }
    this.reserve(4);
    new DataView(this.bytes.buffer).setUint32(this.length, value, false);
    this.length += 4;
  }

  i64(value: number): void {
    if (!Number.isSafeInteger(value)) {
      throw new Error(`identity timestamp must be a safe integer: ${value}`);
    }
    const bytes = new Uint8Array(8);
    new DataView(bytes.buffer).setBigInt64(0, BigInt(value), false);
    this.raw(bytes);
  }

  f64(value: number): void {
    if (!Number.isFinite(value)) throw new Error('identity numbers must be finite');
    const bytes = new Uint8Array(8);
    new DataView(bytes.buffer).setFloat64(0, Object.is(value, -0) ? 0 : value, false);
    this.raw(bytes);
  }

  string(value: string): void {
    const encoded = textEncoder.encode(value);
    this.u32(encoded.length);
    this.raw(encoded);
  }

  finish(): Uint8Array {
    return this.bytes.slice(0, this.length);
  }
}

/**
 * Type-tagged canonical encoding for JSON-compatible values.
 *
 * Object keys sort by UTF-8 bytes, all numbers are finite big-endian IEEE-754
 * f64 values, and -0 normalizes to +0. This avoids JS/Rust JSON-number text
 * differences while preserving array order and exact numeric bits.
 */
export function canonicalBytes(value: unknown): Uint8Array {
  const writer = new ByteWriter();
  writeCanonicalValue(writer, value);
  return writer.finish();
}

function writeCanonicalValue(writer: ByteWriter, value: unknown): void {
  if (value === null) {
    writer.byte(0x00);
    return;
  }
  if (value === false) {
    writer.byte(0x01);
    return;
  }
  if (value === true) {
    writer.byte(0x02);
    return;
  }
  if (typeof value === 'number') {
    writer.byte(0x03);
    writer.f64(value);
    return;
  }
  if (typeof value === 'string') {
    writer.byte(0x04);
    writer.string(value);
    return;
  }
  if (Array.isArray(value)) {
    writer.byte(0x05);
    writer.u32(value.length);
    for (const item of value) writeCanonicalValue(writer, item);
    return;
  }
  if (value && typeof value === 'object') {
    const record = value as Record<string, unknown>;
    const keys = Object.keys(record).sort(compareUtf8);
    writer.byte(0x06);
    writer.u32(keys.length);
    for (const key of keys) {
      const child = record[key];
      if (child === undefined) {
        throw new Error(`identity value at ${key} is undefined`);
      }
      writer.string(key);
      writeCanonicalValue(writer, child);
    }
    return;
  }
  throw new Error(`unsupported identity value: ${typeof value}`);
}

function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const length = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

/** SHA-256 over bytes. Missing Web Crypto is a hard failure for durable IDs. */
export async function sha256BytesHex(bytes: Uint8Array): Promise<string> {
  const subtle = (globalThis.crypto as Crypto | undefined)?.subtle;
  if (!subtle) throw new Error('SHA-256 is unavailable; durable identity creation failed closed');
  const digest = await subtle.digest('SHA-256', bytes as BufferSource);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
}

export async function sha256Hex(value: string): Promise<string> {
  return sha256BytesHex(textEncoder.encode(value));
}

/** Explicitly ephemeral 64-bit FNV-1a fingerprint; never persist or reuse it. */
export function ephemeralFingerprint(value: string): string {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = 0xffffffffffffffffn;
  for (const byte of textEncoder.encode(value)) {
    hash ^= BigInt(byte);
    hash = (hash * prime) & mask;
  }
  return `ephemeral-fnv1a:${hash.toString(16).padStart(16, '0')}`;
}

/** Legacy export retained for callers; its prefix makes non-durability explicit. */
export const fnv1a = ephemeralFingerprint;

export interface ExecModel {
  feePct: number;
  slippagePct: number;
  [key: string]: unknown;
}

/** Durable strategy definition + execution-model identity. */
export async function strategyHash(definition: unknown, execModel: ExecModel): Promise<string> {
  const preimage = concatBytes(
    textEncoder.encode(`${STRATEGY_HASH_VERSION}\0`),
    canonicalBytes({ definition, execModel }),
  );
  return `${STRATEGY_HASH_VERSION}:${await sha256BytesHex(preimage)}`;
}

/** Recompute the persisted params/blocks/code identity from its stored JSON. */
export async function strategyHashFromDefinitionJson(definitionJson: string): Promise<string> {
  const definition: unknown = JSON.parse(definitionJson);
  if (!definition || typeof definition !== 'object' || Array.isArray(definition)) {
    throw new Error('strategy definition must be a JSON object');
  }
  const record = definition as Record<string, unknown>;
  if (typeof record.feePct !== 'number' || typeof record.slipPct !== 'number') {
    throw new Error('strategy definition must contain numeric feePct and slipPct');
  }
  return strategyHash(definition, {
    feePct: record.feePct,
    slippagePct: record.slipPct,
  });
}

/** Ephemeral-only sync fingerprint for hot paths; it is never a durable ID. */
export function strategyHashSync(definition: unknown, execModel: ExecModel): string {
  return ephemeralFingerprint(canonicalize({ definition, execModel }));
}

export interface DatasetMeta {
  exchange: string;
  symbol: string;
  interval: string;
}

export interface DatasetCandle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

function normalizeIdentityNumber(value: number): number {
  if (!Number.isFinite(value)) throw new Error('dataset OHLCV values must be finite');
  return Object.is(value, -0) ? 0 : value;
}

/** Return an immutable canonical order; duplicate timestamps fail closed. */
export function normalizeDatasetCandles(candles: readonly DatasetCandle[]): DatasetCandle[] {
  if (candles.length === 0) throw new Error('no candles to import');
  if (candles.length > 0xffff_ffff) throw new Error('dataset candle count exceeds u32');
  const normalized = candles.map((candle) => ({
    timestamp: candle.timestamp,
    open: normalizeIdentityNumber(candle.open),
    high: normalizeIdentityNumber(candle.high),
    low: normalizeIdentityNumber(candle.low),
    close: normalizeIdentityNumber(candle.close),
    volume: normalizeIdentityNumber(candle.volume),
  })).sort((a, b) => a.timestamp - b.timestamp);
  for (let i = 0; i < normalized.length; i++) {
    if (!Number.isSafeInteger(normalized[i].timestamp)) {
      throw new Error(`dataset timestamp must be a safe integer: ${normalized[i].timestamp}`);
    }
    if (i > 0 && normalized[i - 1].timestamp === normalized[i].timestamp) {
      throw new Error(`duplicate candle timestamp: ${normalized[i].timestamp}`);
    }
  }
  return normalized;
}

function requireMetadata(value: string, field: string): string {
  if (!value || value.trim() !== value) {
    throw new Error(`dataset ${field} must be non-empty and already trimmed`);
  }
  return value;
}

/** Exact dataset-content preimage shared with Rust. */
export function datasetIdentityBytes(
  meta: DatasetMeta,
  candles: readonly DatasetCandle[],
): Uint8Array {
  const normalized = normalizeDatasetCandles(candles);
  return normalizedDatasetIdentityBytes(meta, normalized);
}

function normalizedDatasetIdentityBytes(
  meta: DatasetMeta,
  normalized: readonly DatasetCandle[],
): Uint8Array {
  if (normalized.length === 0) throw new Error('no candles to import');
  for (let i = 0; i < normalized.length; i++) {
    const candle = normalized[i];
    if (!Number.isSafeInteger(candle.timestamp)) {
      throw new Error(`dataset timestamp must be a safe integer: ${candle.timestamp}`);
    }
    if (i > 0 && normalized[i - 1].timestamp >= candle.timestamp) {
      throw new Error('normalized dataset candles must have strictly increasing timestamps');
    }
    normalizeIdentityNumber(candle.open);
    normalizeIdentityNumber(candle.high);
    normalizeIdentityNumber(candle.low);
    normalizeIdentityNumber(candle.close);
    normalizeIdentityNumber(candle.volume);
  }
  const writer = new ByteWriter();
  writer.raw(textEncoder.encode(`${DATASET_HASH_VERSION}\0`));
  writer.string(DATASET_FIELD_MAPPING_VERSION);
  writer.string(requireMetadata(meta.exchange, 'exchange'));
  writer.string(requireMetadata(meta.symbol, 'symbol'));
  writer.string(requireMetadata(meta.interval, 'interval'));
  writer.i64(normalized[0].timestamp);
  writer.i64(normalized[normalized.length - 1].timestamp);
  writer.u32(normalized.length);
  for (const candle of normalized) {
    writer.i64(candle.timestamp);
    writer.f64(candle.open);
    writer.f64(candle.high);
    writer.f64(candle.low);
    writer.f64(candle.close);
    writer.f64(candle.volume);
  }
  return writer.finish();
}

/** Durable identity over metadata plus every timestamp-sorted OHLCV row. */
export async function datasetHash(
  meta: DatasetMeta,
  candles: readonly DatasetCandle[],
): Promise<string> {
  const digest = await sha256BytesHex(datasetIdentityBytes(meta, candles));
  return `${DATASET_HASH_VERSION}:${digest}`;
}

/** Hash rows already returned by normalizeDatasetCandles without sorting twice. */
export async function normalizedDatasetHash(
  meta: DatasetMeta,
  normalized: readonly DatasetCandle[],
): Promise<string> {
  const digest = await sha256BytesHex(normalizedDatasetIdentityBytes(meta, normalized));
  return `${DATASET_HASH_VERSION}:${digest}`;
}
