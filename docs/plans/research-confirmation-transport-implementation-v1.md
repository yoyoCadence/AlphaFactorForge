# Complete witnessed archive v5 implementation order

Date: 2026-10-21
Baseline: merged #202 (`f4b0cd7`).
Authority: [storage/transport §4/7](research-confirmation-storage-transfer-plan-v1.md)
and [witness amendment §7/8](research-source-witness-storage-plan-v1.md).

Keep P13c-3b-2 in two ordered stages, delivered in bounded PRs before the next begins:

1. **3b-2a: complete canonical export.** Add a backend-only v5 exporter with
   one deferred snapshot for actual v4 trial/alpha projection and the complete
   strict witnessed graph. Exact schema-7 header/nine counts, prescribed section
   ordering, all three original blob kinds/text/SHA, permanent unknowns and final
   newline. Bound encoded output while serializing: 256 MiB file, 128 MiB embedded
   v4 text, 4096-byte header, 16 MiB + 4096 other lines; escaping counts. Keep
   v3/v4 public omission guards. No importer, command/provider/UI activation.
   Verify independent Node byte/hash oracles, real managed/manual/witness/unknown
   exports, original bytes/audits, corrupt/incomplete graph refusal, concurrent
   independent snapshot/reopen and exact/one-over encoded bounds. Deliberate
   mutants must fail assertions, then be restored before the complete suite.
2. **3b-2b: strict input and atomic union.** Strict canonical original v5 frame
   parsing, nested v4 source validation and independently reconstructed complete
   source graph precede destination mutation. Reuse caller-owned trial/alpha
   helpers under one immediate transaction, preflight current/proposed state and
   capacities, retain all independent claims/stops/original blobs, derive required
   monotonic conflicts/markers, strictly reload and commit once. Test two hops,
   replacements, duplicate replay, qualification provenance, every resource/file
   guard and late all-table/receipt rollback. No execution/admission capability.

The first export API does not imply a released end-user import/export workflow.

## 2b implementation refinement (2026-10-21)

After merged #203 (`1958e3d`), deliver 2b in two bounded sections:

- **2b-1 strict source validation:** closed canonical original v5 decoding,
  inclusive encoded/decoded limits, exact source/count/hash/order checks, and
  full nested v4 trial/alpha validation. Materialize the exact parsed source
  chain, protocols, receipts, restrictive evidence and alpha rows in a private
  disposable in-memory SQLite database with the existing seven migrations.
  Stage the supplied confirmation inventory without completing conflicts or
  inventing unknown markers; independently reconstruct witnesses and strictly
  reload the complete graph/projections there. No destination is opened or
  locked. A backend-only validation API grants no import/execution rights.
- **2b-2 atomic destination union:** consume the independently validated source,
  preflight actual and complete proposed union before the first destination
  write, then one immediate transaction for trial/alpha/confirmation and final
  strict reload. Preserve the frozen two-hop/replacement/replay, provenance,
  capacity and late rollback requirements above. Record the precise disposable
  proposed-state staging mechanism before implementing it; no destination
  savepoint writes may stand in for preflight.

Each section gets its own verification, draft PR, six exact-head CI checks and
guarded merge before proceeding. This refines granularity, not wire/policy.

Final admission/actual owner-count-share/file authentication remains P13c-4.
Do not change statistical policy, run new calibration/reserved-seed experiments
or expose witnesses to AI context. Existing committed-report reproductions stay.
