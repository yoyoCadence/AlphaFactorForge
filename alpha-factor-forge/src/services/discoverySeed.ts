// RUNNER-CONFIG-001: deterministic candidate sub-seed derivation (`seed-v1`).
//
// PR #66 Resolution D4 forbids deriving seeds from a SQLite run id, a thread
// id, enumeration order, or completion order: none of those are stable across
// a re-import, a resume, or a differently scheduled run. A run stores ONE
// explicit `rootSeed` u32; every per-candidate stream is derived from it with
// a versioned SHA-256 preimage over durable identities only.
//
// Pure: no IO, no state, no runner/DB/thread/event dependency.

import { sha256BytesHex } from '../core/hashing';

/** Version tag written into the preimage; a change here is a contract change. */
export const DISCOVERY_SEED_VERSION = 'seed-v1';

/** Purposes allowed to draw a sub-seed. Unknown purposes fail closed so a typo
 *  can never silently create a second, unreviewed random stream. */
export const DISCOVERY_SEED_PURPOSES = ['random-entry'] as const;
export type DiscoverySeedPurpose = (typeof DISCOVERY_SEED_PURPOSES)[number];

/** Inclusive u32 bounds; `rootSeed` is stored, not derived. */
export const MAX_U32 = 0xffff_ffff;

const textEncoder = new TextEncoder();

export interface DeriveSeedArgs {
  /** Explicit run-level u32 recorded in `discovery-config-v1`. */
  rootSeed: number;
  /** Durable `dataset-content-v2:` identity of the evaluated dataset. */
  datasetContentHash: string;
  /** Durable `strategy-v2:` identity of the candidate. */
  strategyHash: string;
  purpose: DiscoverySeedPurpose;
}

function assertU32(value: number, name: string): void {
  if (!Number.isSafeInteger(value) || value < 0 || value > MAX_U32) {
    throw new RangeError(`${name} must be an integer in [0, 4294967295]`);
  }
}

/** `<version>:<64 lowercase hex>`. A bare prefix, a truncated digest, or
 *  uppercase hex is not a usable identity: accepting one would let a malformed
 *  value silently seed a real random stream. */
const DURABLE_DIGEST_PATTERN = /^[0-9a-f]{64}$/;

function assertIdentity(value: string, name: string, prefix: string): void {
  const marker = `${prefix}:`;
  const digest = typeof value === 'string' && value.startsWith(marker)
    ? value.slice(marker.length)
    : null;
  if (digest === null || !DURABLE_DIGEST_PATTERN.test(digest)) {
    throw new RangeError(`${name} must be a durable ${prefix} identity`);
  }
}

/**
 * Exact `seed-v1` preimage, shared byte-for-byte with the Rust port:
 *
 *   "seed-v1" 0x00
 *   u32be(rootSeed)
 *   u32be(len) utf8(datasetContentHash)
 *   u32be(len) utf8(strategyHash)
 *   u32be(len) utf8(purpose)
 *
 * Length-prefixed strings keep the concatenation unambiguous, so no pair of
 * different inputs can produce the same bytes.
 */
export function discoverySeedPreimage(args: DeriveSeedArgs): Uint8Array {
  assertU32(args.rootSeed, 'rootSeed');
  assertIdentity(args.datasetContentHash, 'datasetContentHash', 'dataset-content-v2');
  assertIdentity(args.strategyHash, 'strategyHash', 'strategy-v2');
  if (!DISCOVERY_SEED_PURPOSES.includes(args.purpose)) {
    throw new RangeError(`unsupported seed purpose "${args.purpose}"`);
  }

  const parts: Uint8Array[] = [];
  parts.push(textEncoder.encode(`${DISCOVERY_SEED_VERSION}\0`));
  const rootSeedBytes = new Uint8Array(4);
  new DataView(rootSeedBytes.buffer).setUint32(0, args.rootSeed, false);
  parts.push(rootSeedBytes);
  for (const value of [args.datasetContentHash, args.strategyHash, args.purpose]) {
    const encoded = textEncoder.encode(value);
    const length = new Uint8Array(4);
    new DataView(length.buffer).setUint32(0, encoded.length, false);
    parts.push(length, encoded);
  }

  const total = parts.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

/** The first four digest bytes read big-endian; the same u32 the Rust port
 *  produces, and directly usable as a `mulberry32` seed. */
export async function deriveDiscoverySeed(args: DeriveSeedArgs): Promise<number> {
  const digest = await sha256BytesHex(discoverySeedPreimage(args));
  return Number.parseInt(digest.slice(0, 8), 16) >>> 0;
}
