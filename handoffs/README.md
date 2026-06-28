# Handoffs

Tracked, async handoff notes between agents (Codex, Claude Code) and humans.

A handoff captures cross-session / cross-agent context that does **not** belong
in code or commit messages — PR reviews, design decisions, and task handoffs —
so the next contributor can continue without relying on private chat memory.
This complements `AGENTS.md` §6 (Handoff Friendliness).

These files are committed to the repo (not local-only), so they are shared and
reviewable like any other doc.

## When to write one

- A PR review with findings someone else must act on before merge.
- A decision (and its rationale) that future work depends on.
- Handing a multi-session task to another agent/human mid-flight.
- Anything you would otherwise only know from this chat session.

Small, self-contained changes do not need a handoff — a clear PR description is
enough.

## Location & naming

- Location: `handoffs/` (this directory).
- Filename: `YYYY-MM-DD-<topic>-vN.md`
  - `topic` is short kebab-case (often `pr<N>-<area>`), `vN` bumps on revisions.
  - Example: `2026-06-28-pr2-ui-port-slice1-review-v1.md`.

## Lifecycle

1. **Open** — author writes the handoff; set a `Status` line (e.g.
   `Needs one small fix before merge`).
2. **Resolution** — whoever acts on it **appends** a `## Resolution` section
   (what changed, commit, verification) and updates `Status`. Do **not** delete
   or rewrite the original content — handoffs are an append-only record.
3. Cross-link with the relevant PR (comment on the PR; reference the file path).

Keep handoffs as a historical trail; do not prune resolved ones.

## Template

```markdown
# Handoff: <title>

Date: YYYY-MM-DD
Repo: <owner>/<repo>
Branch: <branch>
PR: #<n>            (if any)
Status: <open question / needs fix / informational / resolved>

## Summary

<1–3 sentences: what this is and why it matters.>

## Required Action / Decision

<Concrete, ordered asks. Quote code with file + line where relevant.>

## Review Notes

<Optional: observations that are fine as-is, context, trade-offs.>

## Verification

<Commands run and their results, e.g. npm test 37/37, CI status.>

## Resolution (added when acted on)

<What changed, commit hash, re-verification. Appended — original kept intact.>
```
