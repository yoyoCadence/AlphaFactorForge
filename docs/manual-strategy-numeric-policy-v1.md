# Manual strategy numeric policy v1

Date: 2026-10-07
Status: Implementation and local verification complete; PR publication and final-head CI/merge pending under the maintainer's authorization.
Task: NUMERIC-JSON-002a; parent NUMERIC-JSON-002 retains the broader research/runtime rollout.

## Problem and scope

The default serde_json parser can read a legal decimal to a different f64 from JavaScript. The verified manual strategy save then rejects the frontend's identity. Changing the global parser would reinterpret existing strategies, runs and research evidence.

This slice fixes new manual params/blocks/code saves and verified loading of existing saved strategies. It introduces a policy carried by the original definition JSON string, preserves legacy identities, and records explicit copies through the existing parent_strategy_id. It does not reinterpret discovery config v1–v3, alter seed/enumeration/validation contracts, change dataset transport, or rewrite historical research artifacts/calibration results. The audit's other transport/replay boundaries remain in the parent task.

## Version and identity

New manual definitions retain their existing strategy fields and add exactly these required policy markers:

```json
{
  "definitionVersion": "manual-strategy-definition-v1",
  "numericPolicy": "json-f64-roundtrip-v1"
}
```

Both markers are inside the definition hashed by the existing `strategy-v2` binary encoder. Its algorithm/prefix remains unchanged; the document version and numeric policy are part of the hashed input, so marked definitions cannot silently reuse an unmarked legacy identity. The encoder's existing finite-f64 and negative-zero normalization rules remain authoritative.

Dispatch is explicit:

| Original definition | Interpretation |
| --- | --- |
| Both markers absent | `serde-json-default-v1`: existing default serde_json behavior |
| Both exact supported markers | `json-f64-roundtrip-v1`: parse original number tokens with Rust's correctly rounded f64 parser |
| Either marker missing, non-string, or unsupported | Refuse the definition |

New parsing must operate on the original JSON string, not a reserialization of a Value already read with the legacy parser. The manual save invoke already carries original_definition_json as a string, preserving its inner tokens through Tauri and the service transport.

The isolated parser may enable serde_json's `raw_value` support to retain tokens; it must not enable `float_roundtrip` or `arbitrary_precision` globally. New documents reject duplicate object keys, malformed syntax, non-finite numeric interpretations and excessive nesting. Legacy documents retain existing duplicate-key and parsing behavior. Number tokens inside string literals remain strings.

## Saving and reading

- New manual builders validate the editor's strategy, append the supported markers, and calculate the identity over that complete definition and its execution costs.
- Backend hash/mode verification uses the declared policy before opening the strategy write transaction. A mismatched identity or unsupported policy writes nothing.
- The original JSON text is stored unchanged. Reading a row does not migrate, rehash or replace it.
- A separate async `prepare_saved_strategy(strategyId)` read verifies the stored row under its declared policy, then returns its source id/hash, policy name, and the interpreted definition as a JSON **string**. Serialized f64 tokens preserve the resolved bits when the frontend parses that string.
- A missing, malformed or unverifiable row remains visible through the existing library list but cannot be prepared for execution. The frontend does not infer a policy by trying multiple hashes.
- The frontend verifies the preparation response belongs to the currently selected source row. A late response cannot load an older selection.

## Additive copies and lineage

Loading preserves the source row separately from editable strategy state. The editor receives only strategy fields, keeping the discovery configuration boundary unchanged.

When the built definition has the same identity as a loaded marked source, a routine save retains the source's original definition and immutable parent relationship. Rename/source updates keep the existing UPSERT semantics.

A changed definition or a legacy-to-marked conversion is an explicit **save as a new version** action. It creates a new identity/row and sets parent_strategy_id to the source row id; the source's original JSON, hash, lifecycle, summaries, trades, attempts and artifact/trial links remain unchanged. The run and save use the same captured source provenance.

The current schema stores one immutable parent per row. If the destination identity already exists with a different parent, reject the copy before writes instead of claiming a lineage that the existing row does not contain. Do not silently replace that parent or add a new schema just to merge copy histories. Self-parenting is refused. This limitation is surfaced as an actionable save error; the user can load the existing strategy.

No bulk or automatic conversion is performed. Unchanged legacy research runs and artifacts remain associated with their original identities and parser interpretation.

## Acceptance

1. Independent frontend literal/bit/hash evidence agrees with the isolated backend parser, including the known `0.0036944444444444438` counterexample, exponent/subnormal values and nested arrays/objects.
2. New params/blocks/code definitions save and read through the real migrated SQLite repository with exact original text and the frontend hash.
3. The legacy audit's six cases retain their original hashes/behavior; verified edit preparation returns the backend's original interpreted f64 values.
4. Unknown/partial markers, duplicate keys in new documents, forged identities and unverifiable legacy rows fail without writes.
5. Explicit copies preserve source data and parent lineage; a conflicting existing destination/self-parent is refused atomically. Ordinary repeated saves preserve immutable metadata/lifecycle.
6. UI late preparation, source capture, load → run → save-as-new-version and a repeated save are covered without weakening existing context guards.
7. A real isolated Tauri invoke exercises the adversarial decimal save/read/prepare boundary; mock success alone does not qualify that transport.
8. Existing legacy fixtures/calibration evidence remain unchanged, and the relevant frontend, Rust, build and native CI checks pass.

## Remaining rollout

The parent NUMERIC-JSON-002 still owns correctly rounded new discovery configurations/admission/resume, policy-preserving idempotent request transport, dataset import transport and versioned research artifact replay. Those need explicit new carriers/contracts and preserved legacy adapters; this manual slice does not claim they have been repaired. P12/P13 status and operator credentials/permissions remain independent.

## Implementation evidence

Full verification: 1131 Vitest / 70 files, 600 Rust (170 library + 428 desktop + 2 service, 1 ignored), 85 browser E2E, typecheck, frontend/native debug build and all-target cargo check pass. Real isolated Tauri IPC passes 18 marked decimal/mode cases plus preserved legacy interpretation, additive copy lineage and rejected-policy/parent/argument checks. Historical audit fixtures and research/validation/calibration contracts are unchanged. See [handoff](../handoffs/2026-10-07-manual-strategy-numeric-policy-v1.md) for evidence and limits.
