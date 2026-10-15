# Alpha-aware workspace binding v1

P13b-2a implements the binding part of the [persistence plan](plans/research-reservation-registry-plan-v1.md)
on the [external alpha storage](research-alpha-registry-storage-v1.md).
It extends the existing workspace watermark, without a database migration or
new command. Complete alpha transfer is P13b-2b; confirmation admission,
exposure and recovery are P13c.

## Stored form

`app_settings.trial_ledger_binding` now writes `trial-ledger-binding-v3`:

```json
{
  "version": "trial-ledger-binding-v3",
  "registryId": "<32 lowercase hex>",
  "seq": 0,
  "chainHead": "<64 lowercase hex>",
  "evidence": {
    "quarantinedFamilies": [],
    "conflictedOrigins": [],
    "familyTests": {}
  },
  "alpha": {
    "budgets": [],
    "reservations": [],
    "conflicts": []
  }
}
```

The event prefix and restrictive evidence retain trial-ledger §7/§23 rules.
Each budget entry is `{familyId, allocationId}`, ordered lexicographically
by that pair. A declaration can legitimately be shared by different families;
an allocation ID alone would miss loss or substitution of its family binding.
Alternative declarations retained by permanent budget conflicts also enter
this set; multiple declarations do not resolve or release the conflict.
Reservations are reservation IDs;
conflicts are `alpha-registry-conflict-v1` content IDs. All three arrays must
be sorted and duplicate-free. IDs use their existing strict formats.

The complete binding text is bounded to **4 MiB** on read and before writes,
including legacy formats. The three alpha sets together may contain at most
**30,000 identities**: storage permits 10,000 total rows, and each conflict
can retain two budget identities in addition to its own ID. A shared
conflict occupies one identity, but its complete affected-family row set
must still validate. Exceeding a limit is an error, never truncation. A
workspace with an unusually large prior binding must keep that evidence and
resolve capacity before adoption; the reader never replaces it with empty
history. Unknown/missing/null alpha fields, malformed sets and duplicate
struct fields are refused. Parsing validates the original JSON text rather
than discarding duplicate struct fields through a JSON-value conversion.

## Observation and coverage

`check_binding` verifies the event prefix, §23 evidence and alpha identities
inside **one deferred registry transaction**. Alpha observation first runs the
bounded storage reader: declaration IDs and canonical documents, known trial
family bindings, reservation hashes/body/declared shares, conflict hashes and
every affected-family conflict copy must agree. Selecting raw SQL IDs is
insufficient. A damaged record with an unchanged ID cannot satisfy coverage.

Every saved budget pair, reservation ID and conflict ID must remain present.
Growth is accepted; equal counts with different valid identities are refused
as `registry_evidence_rolled_back`. Malformed current evidence returns
`registry_state_invalid`. Neither path overwrites the workspace's last good
binding. Existing workspace adoption and owner-checked enqueue writes persist
the returned complete v3 observation in their existing transactions. As with
earlier binding failures, invalid adoption stops opening the workspace until
the registry evidence is restored; this slice adds no recovery bypass.

Valid alpha conflicts remain distinct from ordinary trial quarantines:
adoption and exploratory trial registration still work while their complete
conflict evidence is present. Confirmation remains blocked by alpha history.
The existing admission fence remains an event-prefix/count observation, not
an atomic alpha reservation or confirmation permit.

## Legacy and transfer limits

Unversioned v1 retains unknown restrictive and alpha evidence. v2 retains its
§23 evidence and **unknown alpha evidence** (`None`), never known-empty sets.
Both may be read to prove their event prefix; neither can be written directly
by the new writer. Successful verified adoption observes the actual registry
and writes v3, including existing alpha rows. Failed adoption keeps the old
binding. Older v2 readers reject v3 rather than overwrite its unknown fields.

The watermark proves coverage of observed evidence, not completeness of an
unseen registry's history. A trial-only v3 export with a valid imported prefix
cannot replace saved nonempty alpha evidence. Such exports still cannot prove
P13 qualification history even when this watermark was empty: complete
alpha-aware transfer and qualification refusal of older transfer versions
remain P13b-2b. No binding settles `legacy_exposure_unknown`, validates an
unsupported statistical configuration or authorizes any data reveal.
