# Original market consumer seed-v1 correction v2

Date: 2026-10-31 (UTC+8). Baseline: verified #240 (`8d12f6a`).
Bounded design/data correction before whole-consumer implementation continues.

Original-writer setup for the [market contract](research-original-market-consumer-v1.md)
failed before invoking the new reader: the unchanged producer returned
randomEntry `3642945820`, while the v1 input claimed `3949539066`. The v1 author
rebound dataset/config/applicability identities but copied the old seeds. The
old independent checker made the same assumption and therefore missed the defect.
Neither the producer nor the reader should accommodate that inconsistent input.

The [v1 artifact](../../alpha-factor-forge/fixtures/research/original-market-consumer-v1.json)
remains byte-identical (LF SHA256
`f9127cc5c0abb09452f6eff721bd95137df5fa03061f7e325ff7772827f1beb8`).
The [additive v2 artifact](../../alpha-factor-forge/fixtures/research/original-market-consumer-v2.json)
changes only dataset-dependent seed tokens, their raw fingerprint SHA256s and
the resulting original parse/scratch byte counts, plus explicit correction
metadata. Market/campaign/config/hypothesis identities, lineage/warmup outputs,
all nodes/expansion counts, post-preparation W/C/A/Q/O and limits stay identical.
Original producers, runtime, schema, dependencies and all older fixtures stay
unchanged. This does not close actual4c or confer execution/source authority.

The original [seed implementation](../../alpha-factor-forge/src-tauri/src/discovery_core/seed.rs)
frames UTF-8 `seed-v1`, NUL, rootSeed u32 big-endian, then three u32 big-endian
length-prefixed UTF-8 strings: datasetHash, strategyHash, `random-entry`.
Each frozen frame is 195 bytes. Interpret the first four SHA256 bytes as u32
big-endian. The v2 artifact freezes all14 family frames/digests, old/new seeds
and decimal width deltas. Replace only the exact seed number token in original
inputFingerprintJson; retain `.0`, whitespace, field ordering and every other
original byte. Recompute each raw fingerprint SHA256.

| Family | Correct randomEntry seeds, original candidate order | Parse/scratch delta |
| --- | --- | --- |
| captured-old-revision | 3642945820, 3418825277, 3592403324 | 0 |
| new-revision-mixed | 3642945820, 3418825277, 3592403324, 2562401537 | +1 |
| forward-offset-availability | 3642945820, 3418825277, 3592403324 | 0 |
| etf-declared-references | 2218024905, 1584550661, 1767743226, 3222358544 | +1 |

Each mixed family's DSL seed grows from nine to ten decimal digits. Corrected
prior P/scratch are17647/19769 for both mixed families. Corrected joint
P/scratch are19005/21127 for crypto mixed and18992/21114 for ETF mixed.

Resource seeds come from **original definitions before stored-row poisoning**.
Reconstruct the frozen raw grids, prune by the original three cross-field rules,
deduplicate by original strategy-v2 identity and sort that identity before
indexing/seeding. The original binary identity tags/UTF-8 order/f64 numbers are
defined in [identity.rs](../../alpha-factor-forge/src-tauri/src/discovery_core/identity.rs),
with enumeration order in [enumerate.rs](../../alpha-factor-forge/src-tauri/src/discovery_core/enumerate.rs).
Freeze all1112 original candidate identities/frames/digests/seeds and exact
original config documents, rather than deriving seeds from the later poisoned
stored definitions. Output exact/over differ only in other-base ID length;
strategy identity and seeds are equal for both variants.

| Resource | Candidates | Seed decimal bytes delta | Exact joint P/scratch | Over joint P/scratch |
| --- | ---: | ---: | --- | --- |
| hash | 8 | +1 | 31770220 / 31774892 | 31770221 / 31774893 |
| analysis | 2 | 0 | 4183064 / 4184049 | 4183065 / 4184050 |
| copy | 1 | 0 | 1685731 / 1686190 | 1685732 / 1686191 |
| output | 1101 | +13 | 5799784 / 10452739 | 5799788 / 10452744 |

The existing exact/+1 **post-work** boundaries need no padding adjustment: seed
text contributes to preparation P, while the frozen later work excludes it.
Exact hash/analysis/copy reserve then refuse their intentional original-identity
poison; exact output must still complete every candidate, and over refuses before
payload work. Clone/resource guarantees and all negative controls remain binding.

Reproduce from repository root with stdlib Python and Node built-ins:

```powershell
py alpha-factor-forge/scripts/generate-original-market-consumer-v2.py
node alpha-factor-forge/scripts/verify-original-market-consumer-v2.cjs
```

The author imports no product code. The separate read-only verifier implements
strategy-v2 and seed-v1 independently, rederives original producer seed literals
before rebinding, verifies every new seed/frame/digest, preserves raw token
formatting and compares the entire normalized v2 artifact with immutable v1.
It also retains the original market/catalog/work checks. Historical source pins
are checked at their recorded v1 baseline; seed/identity/enumeration pins are
checked against unchanged current sources. On Windows the sandbox may block
Node's read-only `git show` child process with EPERM; rerun that exact verifier
with escalation, without changing artifacts or source checks.

After independent review, six exact-head/base CI jobs and verified normal merge,
rebase unpublished consumer checkpoint `d682a8d` onto the corrected baseline.
Use v2 in original-producer controls, then complete the remaining consumer
negative/boundary/mutation controls and full delivery gates. The original failed
setup log stays evidence; never use consumer output to author expected values.
