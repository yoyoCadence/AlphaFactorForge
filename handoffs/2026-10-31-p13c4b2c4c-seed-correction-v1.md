# Handoff: original market seed fixture correction

Date: 2026-10-31 (UTC+8)
Repo: yoyoCadence/AlphaFactorForge
Branch: docs/original-market-consumer-seed-correction
Status: Independent review approves; exact-head CI/normal merge pending.

## Scope and original contradiction

After verified #240 (`8d12f6a`), original-writer setup for whole-consumer4c-2
failed before new observer invocation: old seed3949539066 versus correct
3642945820. seed-v1 includes dataset identity. The v1 author and its checker
both copied the old dataset's seeds while rebinding the other identities.
Initial log: ignored test-results/market-consumer/initial.log (3 pass/1 fail).
The failing file-backed test closes/removes its owned workspace on panic; outer
exclusive diagnostic root remains recorded in initial-root.json. No protected
execution or source admission ran. The incomplete implementation is retained
as unpublished local checkpoint `d682a8d` on feat/original-market-consumer.

## Change and continuation

Add v2 without changing v1, producers, runtime, schema or older fixtures. Correct
all14 family seed tokens/raw hashes; independently freeze195-byte frames and
all1112 pre-poison resource candidates. Mixed family P/scratch each+1; hash
resource P/scratch+1, output+13. Nodes/E/W/C/A/Q/O, all other identities and
semantics remain unchanged. [Erratum](../docs/plans/research-original-market-consumer-seed-correction-v2.md)
is binding alongside the original market contract. Track the author and separate
read-only Node verifier so the seed dependency cannot disappear with TEMP files.

After this bounded design/data PR passes review/six exact-head/base jobs and
normal merge/readback, rebase the unpublished consumer, use v2 and finish all
actual controls before its own PR/merge. Actual4c/source/live parents stay open.

## Verification

Independent reviewer Python hashlib/struct and Node built-ins agree across14
family inputs and1112 resource candidates, including four sorted seed manifest
hashes. They derive original strategy identities and seeds from original frozen
config templates before any stored-row resource poison; no product execution or
new reader is an oracle. The tracked Python author and separate tracked Node
verifier agree. Node read-only git-show child EPERM required the exact command
with escalation, which passed unchanged. Reproducibility, verifier negative
controls, full artifact preservation/scope, links and append-only receipts are
recorded below before publication. Runtime suites run on final-head CI; no local
runtime change is introduced by this correction.

## Local verification receipt

V2 is1029883 LF bytes, SHA256
`a12f373f2a13580263ccb752bf6be773b812e457baa259b2646810a501aabf01`.
Repeating the Python author reproduces exact bytes. The final Node verifier
passes unchanged; six deliberate invalid artifacts each fail an assertion:
stale family seed, resource seed substitution, pre-poison config substitution,
missing manifest member, old preparation count and unrelated market identity
change. Controls and assertion logs remain ignored under
test-results/market-seed-correction, invalid copies in an exclusive GUID TEMP
directory. No product source is mutated for these data-verifier controls.
All older fixture bytes/runtime scope, local links, diff checks and append-only
#240 resolution pass. The previous captured-range handoff now records verified
#240 delivery; its historical body is preserved.

The separate reviewer session approves correctness/scope after independently
normalizing the complete v2 back to v1 and checking all14/1112 original identities,
seed vectors and byte deltas. It reproduced exact tracked Node validation and
confirmed all3 current and6 historical source pins. A nonblocking literal-question-
mark separator issue in new task/receipt headings was corrected before commit.
