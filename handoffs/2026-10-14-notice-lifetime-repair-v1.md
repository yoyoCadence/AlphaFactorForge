# Handoff: PR #173 repeated notice lifetime repair

Date: 2026-10-14
Repo: yoyoCadence/AlphaFactorForge
Branch: `fix/notice-emission-lifetime`
Status: In Progress; development, publication and merge authorized.

## Scope

Acceptance R3: repeating the same sweep cell at seven seconds emits identical
text, but must start a new eight-second lifetime. The notice now has a fresh
object identity for every emission. The string/null callback remains stable;
timer cleanup and an identity guard protect newer notices. Message text,
dismiss controls and persistent error behavior stay the same.

## Verification

The added clock E2E fails on the original #173 code two seconds after repeating
the message. It also checks visibility seven seconds after re-emission and
expiry after eight seconds. Fixed code passes all three status-message E2E
cases, 1,170 Vitest tests, typecheck and production build at base `63bc2df`.
Rebase and integrated checks with #176 follow before publication.

## Resolution

Pending verification, rebase onto merged #176 and publication.
