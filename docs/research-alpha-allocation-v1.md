# Research alpha allocation (`research-alpha-allocation-v1`)

> **Status: P12e-2, pure calculation only (2026-10-01).** No runtime caller.
> It says how much alpha a trial family's next confirmation may use; it never
> reserves that alpha, stores anything, or produces a confirmation `PASS` —
> P13 owns those. Implementation:
> `alpha-factor-forge/src-tauri/src/discovery_core/alpha_allocation.rs`.
> Upstream: [`plans/active-plan.md`](plans/active-plan.md) §4.5 ("跨確認批次採
> 預先登記的 alpha spending");
> [`research-confirmation-statistics-v1.md`](research-confirmation-statistics-v1.md)
> (P12e-1, which consumes the `alphaPpm` allocated here);
> [`research-precision-v1.md`](research-precision-v1.md) (P12a);
> [`trial-ledger-v1.md`](trial-ledger-v1.md) §3 (trial families).

## 0. Maintainer decisions (2026-10-01)

1. **A declared schedule.** The budget is a total plus the alpha of the
   first, second, third… confirmation, written down beforehand. An equal split
   is just one such schedule; a helper generates it, flooring and never
   rounding up (§4).
2. **One budget per trial family** (one instrument) — the same scope as the
   trial count. A new workspace or a new campaign does not get a new budget.
   Once it is used up, that instrument cannot be confirmed again under this
   contract version.

## 1. Question it answers

Given a family's frozen alpha budget and the confirmations it has already
reserved: may it run another confirmation, and with exactly which `alphaPpm`?

It does not decide what total is appropriate, whether a candidate is
confirmed, or whether the reserved history it was given is complete.

## 2. Declaration (strict JSON)

All fields are required; unknown fields are rejected; every number is a JSON
integer (a float literal such as `20000.0` or `16666.5` is rejected).

| Field | Domain | Meaning |
| --- | --- | --- |
| `contractVersion` | `"research-alpha-allocation-v1"` | exact |
| `rule` | `"declared-schedule"` | only rule in v1 |
| `scope` | `"trial-family"` | only scope in v1 (decision 2) |
| `totalAlphaPpm` | integer `[1, 999999]` | family-wise alpha for **every** confirmation the family will ever run, parts per million (`0.05` = `50000`) |
| `schedule` | non-empty array of integers `[1, 999999]` | entry `k` (1-based) is the `alphaPpm` of the family's k-th confirmation; the entries must sum to at most `totalAlphaPpm` |

Rejection order (part of the contract): not an object → first unknown field in
sorted order → the fields in table order, schedule entries in array order
(`allocation.schedule[i]`, 0-based) → the schedule's sum against the total.

The bound is exact: a sum equal to the total is accepted, one ppm over is
refused. No field has a default and no tolerance is applied. There is no
separate limit on the number of confirmations; since every share is at least
1 ppm, it cannot exceed `totalAlphaPpm`.

## 3. Allocation

`allocate_confirmation_alpha(declaration, reserved)` — `reserved` is the
`alphaPpm` of every confirmation the family has already reserved, oldest
first, **including ones that failed or never finished**
([`active-plan`](plans/active-plan.md) §4.4: a reservation survives a failed
run).

1. The declaration is re-checked against §2 even when constructed directly.
2. `reserved` must be exactly the first `reserved.length` schedule entries.
   More reservations than scheduled confirmations, or any entry that differs
   from the schedule (a different amount, or the same amounts in another
   order), is an **error** — a contradicted history is never turned into an
   allocation. Length is checked before content.
3. If a scheduled entry remains, the next confirmation gets it: `ELIGIBLE`
   with `confirmationNumber = reserved.length + 1` and that `alphaPpm`.
4. Otherwise `NOT_ELIGIBLE` with reason `alpha_budget_exhausted` and no
   `alphaPpm`.

Nothing is carried over or recycled: a confirmation that rejected no
hypothesis, failed, or was abandoned has still spent its share, and a share is
never enlarged by what earlier confirmations "did not use".

## 4. Equal split

`equal_alpha_schedule(totalAlphaPpm, confirmations)` returns `confirmations`
entries of `floor(totalAlphaPpm / confirmations)`.

- The remainder stays **unscheduled**. It is not added to any entry and cannot
  be used later (`50000 / 3` → three shares of `16666`, 2 ppm unused).
- `confirmations` must be in `[1, totalAlphaPpm]`; a split whose share would
  floor to zero is refused instead of producing a zero share.

The result is an ordinary schedule: the declaration always stores the explicit
list, never "split into N".

## 5. Identity

`allocationId` is the lowercase hex SHA-256 of

```text
"research-alpha-allocation-v1" || 0x00 || canonical_bytes(document)
```

where `document` is the five fields of §2 and `canonical_bytes` is the
type-tagged encoding shared with TypeScript (`discovery_core::identity`,
`src/core/hashing`) — the campaign declaration's construction. Schedule order
is part of the identity; the reserved history is not.

## 6. Report

camelCase JSON: `contractVersion`, `rule`, `scope`, `allocationId`, `status`
(`ELIGIBLE` / `NOT_ELIGIBLE`), `reasons`, `totalAlphaPpm`,
`scheduledConfirmations`, `reservedConfirmations`, `spentAlphaPpm` (before
this confirmation), `confirmationNumber` and `alphaPpm` (`null` when
exhausted), `remainingConfirmations` and `remainingScheduledAlphaPpm` (after
this confirmation), `unscheduledAlphaPpm`.

Every ppm is accounted for once (tested):

```text
spentAlphaPpm + (alphaPpm or 0) + remainingScheduledAlphaPpm + unscheduledAlphaPpm
  = totalAlphaPpm
```

`ELIGIBLE` means only that the family still has a scheduled share. It is not
evidence about any strategy, and there is no verdict or `PASS` field.

## 7. Why the total holds

Each confirmation controls its own family-wise error at its share `α_k`
(P12e-1's Holm against the whole family). The chance that **any** of the
family's confirmations makes a false rejection is at most `Σ α_k` (union
bound), and the schedule's sum is at most `totalAlphaPpm`. This needs no
assumption about how confirmations depend on each other, which is why shares
are fixed in advance and never recycled.

A smaller share is a stricter test and needs more bootstrap samples to be
resolvable at all: P12a's `B ≥ ceil(1e6·m / alphaPpm) − 1`. With ten family
tests, 50,000 ppm needs 199 samples and 5,000 ppm needs 1,999 (tested). The
precheck therefore has to be run with the allocated share.

## 8. Fixture and reference

[`research-alpha-allocation-v1.json`](../alpha-factor-forge/fixtures/rs-core/research-alpha-allocation-v1.json)
holds hand-written declarations, reserved histories and error messages. Its
`expected` reports (including `allocationId`) and equal-split schedules are
produced by an independent TypeScript reference written from this document
(`src/parity/alphaAllocationFixture.ts`, `npm run fixtures:alpha-allocation`),
not by the Rust module. A Vitest fails if the committed fixture stops matching
the reference; the Rust tests must reproduce the fixture exactly.

Cases: a four-step schedule at its first, third and last confirmation and
after exhaustion; an equal split that leaves 2 ppm unscheduled; one
confirmation taking the whole budget; a schedule one ppm above the total; a
zero share; a reserved amount that differs from the schedule; more
reservations than scheduled confirmations. 50,000 ppm is a fixture choice, not
a product default.

## 9. What P13 must do (this module cannot)

The function is pure and trusts its inputs. For the budget to mean anything:

1. **One declaration per family, kept with the trial registry** (outside the
   workspace), fixed before the family's first confirmation and identified by
   `allocationId`. A different declaration for a family that already has one
   must be refused; otherwise redeclaring is a reset.
2. **Reserve before running**, atomically, and pass *every* reservation of
   the family as `reserved`. A caller that passes a shorter list gets an
   earlier confirmation number again — exactly the reset decision 2 forbids,
   and undetectable here (tested and documented, like `priorTrials` in P12a).
3. **Use the allocated share**: the confirmation declaration's `alphaPpm`
   must equal this report's `alphaPpm`, and the P12a precheck must hold at
   that alpha with the fenced family count.
4. **Reconcile with the campaign.** A campaign's `sampling.alphaPpm`
   ([`research-campaign-declaration-v1.md`](research-campaign-declaration-v1.md))
   is what admission planned with. If it differs from the allocated share,
   P13 has to refuse or re-check; this contract does not choose.
5. **Union on export/import.** Declarations and reservations must travel
   with the ledger. If two registries each reserved "confirmation 1" for the
   same family, the merged history contradicts the schedule and §3 stops
   allocating. v1 has no rule for resolving that; it fails closed.
6. `NOT_ELIGIBLE` here blocks confirmation only; exploration may continue
   (maintainer decision of 2026-09-29).

## 10. Limits

- **Per-family budgets do not add up to a portfolio guarantee.** Each
  instrument has its own total. Across `N` instruments, the chance of at
  least one false confirmation somewhere can approach `N × total`.
- The total and the shares are declared numbers. Nothing here says which
  total is appropriate or checks that a schedule is sensible; a schedule that
  spends most of the budget first leaves later confirmations very strict.
- Exhaustion is permanent under this version. More confirmations of the same
  instrument would need a new contract version and a maintainer decision, not
  a new declaration.
- Unused alpha is lost by design (§3). That is conservative, not optimal.
- Rust only. The reference in `src/parity` is test support, not a product
  path.

## 11. Remaining P12/P13 work

P12e-3 (seeded noise-data false-positive simulation of the declared
protocol) and P13 (§9, plus freezing a confirmation batch, the synchronized
ledger fence, revealing Validation/Test once, and the only place a
statistical `PASS` may be produced).
