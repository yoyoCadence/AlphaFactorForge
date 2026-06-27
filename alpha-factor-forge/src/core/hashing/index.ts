// FULL — deterministic content hashing for dedup.
// strategyHash(def, execModel) and datasetHash(meta) must be STABLE across
// machines: we canonicalize (sort keys) before hashing so key order can't
// change the result. Uses SHA-256 via Web Crypto (async) with a sync FNV-1a
// fallback for environments without crypto.subtle (e.g. some Worker contexts).

/** Canonical JSON: object keys sorted recursively, no insignificant whitespace. */
export function canonicalize(value: unknown): string {
  return JSON.stringify(sortDeep(value));
}

function sortDeep(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(sortDeep);
  if (v && typeof v === 'object') {
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(v as Record<string, unknown>).sort()) {
      out[k] = sortDeep((v as Record<string, unknown>)[k]);
    }
    return out;
  }
  return v;
}

/** Synchronous, dependency-free 64-bit FNV-1a hash -> hex. Stable fallback. */
export function fnv1a(str: string): string {
  // 64-bit FNV-1a using BigInt for determinism across platforms.
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = 0xffffffffffffffffn;
  for (let i = 0; i < str.length; i++) {
    hash ^= BigInt(str.charCodeAt(i));
    hash = (hash * prime) & mask;
  }
  return hash.toString(16).padStart(16, '0');
}

/** SHA-256 hex if Web Crypto is available, else FNV-1a fallback. */
export async function sha256Hex(str: string): Promise<string> {
  const subtle = (globalThis.crypto as Crypto | undefined)?.subtle;
  if (subtle) {
    const buf = await subtle.digest('SHA-256', new TextEncoder().encode(str));
    return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('');
  }
  return fnv1a(str);
}

export interface ExecModel {
  feePct: number;
  slippagePct: number;
  [k: string]: unknown;
}

/** Hash of a strategy definition + its execution model (dedup key part 1). */
export async function strategyHash(definition: unknown, execModel: ExecModel): Promise<string> {
  return sha256Hex(canonicalize({ definition, execModel }));
}

/** Sync variant (FNV-1a) for hot paths / Workers without crypto.subtle. */
export function strategyHashSync(definition: unknown, execModel: ExecModel): string {
  return fnv1a(canonicalize({ definition, execModel }));
}

export interface DatasetMeta {
  exchange: string;
  symbol: string;
  interval: string;
  startTime: number;
  endTime: number;
  /** bump when the candle source/cleaning changes so old data re-hashes. */
  dataVersion?: number;
}

/** Hash of a dataset's identity (dedup key part 2). */
export async function datasetHash(meta: DatasetMeta): Promise<string> {
  return sha256Hex(
    canonicalize({
      exchange: meta.exchange,
      symbol: meta.symbol,
      interval: meta.interval,
      startTime: meta.startTime,
      endTime: meta.endTime,
      dataVersion: meta.dataVersion ?? 1,
    }),
  );
}
