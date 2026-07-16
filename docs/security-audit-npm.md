# SEC-001 — npm dependency audit triage

Audit date: **2026-07-16 (Asia/Taipei)**

## Executive summary

`npm audit --json` currently reports **5 affected package nodes**: 3 moderate, 1 high, and 1 critical. All five are development-tool dependencies under Vite or Vitest. `npm audit --omit=dev --json` reports **0 production findings**.

This does not make the findings irrelevant. AlphaFactorForge development runs on Windows and starts a local Vite server, which overlaps with several advisory preconditions. No dependency was changed in SEC-001 because the smallest complete remediation crosses the current Vite 5 and Vitest 2 major-version boundaries.

Recommendation: classify all five package nodes as **`needs-window`**, keep the short-term development-server restrictions below, and handle the coordinated Vite/Vitest upgrade as a separate, fully tested task. Do not run any form of `npm audit fix`, especially `npm audit fix --force`.

## Reproducible snapshot

| Item | Value |
| --- | --- |
| Repository commit audited | `bbc9d094d351a9d3150f33d42850b73344386a0d` |
| `package-lock.json` Git blob | `c8b8122d3982a9aa69ebae24c9313f5047efc647` |
| Local runtime | Node `v24.14.1`; npm `11.12.1` |
| Full command | `npm audit --json` |
| Full result | 161 dependencies; 3 moderate, 1 high, 1 critical; total 5 |
| Production-only command | `npm audit --omit=dev --json` |
| Production-only result | 7 production dependencies; total 0 |

The snapshot is time-sensitive: npm advisory metadata can change without a lockfile change. Re-run both audit commands before acting on this report.

## Affected package-node classification

The npm total counts affected nodes in the installed dependency graph. A transitive node can inherit a Vite advisory, so this table should not be read as five unrelated runtime vulnerabilities.

| Package node | Path from project | Severity | Production bundle? | Fix signal from audit | Classification |
| --- | --- | --- | --- | --- | --- |
| `vite@5.4.21` | direct `devDependency` | high | No | npm proposes `vite@8.1.5`, a semver-major change | `needs-window` |
| `vitest@2.1.9` | direct `devDependency` | critical | No | npm proposes `vitest@4.1.10`, a semver-major change | `needs-window` |
| `esbuild@0.21.5` | `vite -> esbuild` | moderate | No | Must be raised through a compatible Vite release; do not override it in isolation | `needs-window` |
| `@vitest/mocker@2.1.9` | `vitest -> @vitest/mocker -> vite` | moderate | No | Inherits Vite findings; npm proposes the Vitest major upgrade | `needs-window` |
| `vite-node@2.1.9` | `vitest -> vite-node -> vite` | moderate | No | Inherits Vite findings; npm proposes the Vitest major upgrade | `needs-window` |

Why none are `safe-now`: the current direct ranges are Vite 5 and Vitest 2, while a complete remediation requires coordinated major upgrades. Why none are `accept-risk`: the vulnerable tools are actively used on a Windows developer workstation, and the graph includes high and critical findings even though their strongest network/UI preconditions are absent in the repository configuration.

## Underlying advisories

| Advisory | Package surfaced by npm | Severity | Relevant condition | Patched line |
| --- | --- | --- | --- | --- |
| [GHSA-67mh-4wv8-2f99](https://github.com/advisories/GHSA-67mh-4wv8-2f99) | `esbuild` | moderate | The vulnerable `serve` behavior can expose development-server responses to a malicious website | `esbuild@0.25.0` |
| [GHSA-4w7w-66w2-5vf9](https://github.com/advisories/GHSA-4w7w-66w2-5vf9) | `vite` | moderate | Optimized-dependency source maps can escape the project when the dev server is exposed to the network | Vite `6.4.2`, `7.3.2`, or `8.0.5` |
| [GHSA-v6wh-96g9-6wx3](https://github.com/advisories/GHSA-v6wh-96g9-6wx3) | `vite` | moderate | On Windows, a crafted open-in-editor UNC path can disclose an NTLMv2 hash while the dev server is running | Vite `6.4.3`, `7.3.5`, or `8.0.16` |
| [GHSA-fx2h-pf6j-xcff](https://github.com/advisories/GHSA-fx2h-pf6j-xcff) | `vite` | high | Windows alternate paths can bypass `server.fs.deny` when the dev server is exposed to the network | Vite `6.4.3`, `7.3.5`, or `8.0.16` |
| [GHSA-5xrq-8626-4rwp](https://github.com/advisories/GHSA-5xrq-8626-4rwp) | `vitest` | critical | Vitest UI/API or Browser Mode can read/write files and execute tests, especially on Windows or when network-exposed | Vitest `3.2.6` or `4.1.0` |

The Vite advisory metadata changed recently. Although the npm JSON returned narrower Vite ranges during this audit, the latest reviewed advisories require **Vite 6.4.3** to cover all three Vite findings. The report uses that stricter floor.

## Repository exposure assessment

- `vite`, `vitest`, and all three transitive nodes are marked development-only in `package-lock.json`; the production-only audit is clean.
- `vite.config.ts` does not opt into `server.host`, and Playwright targets `localhost`. The repository therefore does not intentionally expose the Vite server to the LAN.
- The test script is `vitest run`. There is no repository command or configuration enabling Vitest UI, Browser Mode, or a network API host.
- The workstation is Windows. The UNC/NTLM and alternate-path advisories remain relevant whenever the local development server is running, even though the default project workflow removes important exploitation preconditions.
- The project does not call esbuild's `serve` API directly. Its affected version is present transitively through Vite, so it should be remediated through Vite rather than by forcing an unsupported override.

## Remediation decision

### Recommended next task: minimum patched stack

Test a coordinated upgrade to:

- `vite@6.4.3`
- `vitest@3.2.6`
- keep the installed `@vitejs/plugin-react@4.7.0`, whose peer range includes Vite 6

This is the smallest identified combination that covers the reviewed advisory floors: Vite 6.4.3 depends on `esbuild ^0.25.0`, and Vitest 3.2.6 supports Vite 5/6/7 while replacing its affected Vitest internals. Both support the CI Node 20 line. It still crosses two majors and therefore needs the full typecheck, unit, build, and browser E2E gates in a dedicated PR.

The npm automatic proposal (`vite@8.1.5` + `vitest@4.1.10`) is not the first choice for this remediation. Vite 8 would also require upgrading `@vitejs/plugin-react` and verifying a Node version of at least `20.19.0`, increasing the compatibility surface without being necessary to clear these advisory floors.

### Temporary controls until that task merges

- Do not start Vite with `--host` or add `server.host`.
- Do not enable Vitest UI, Browser Mode, or a network API host.
- Stop local dev/test servers after use; while they run on Windows, avoid browsing untrusted sites.
- Do not add dependency overrides for `esbuild`, `vite-node`, or `@vitest/mocker` independently.
- Never run `npm audit fix` or `npm audit fix --force`; apply explicit versions and review the resulting lockfile.

## Upgrade acceptance gates

The follow-up upgrade is complete only when:

1. `npm audit --json` and `npm audit --omit=dev --json` both report zero findings for the new lockfile.
2. `npm run typecheck`, `npm test`, `npm run build`, and `npm run e2e` pass.
3. The Vite dev server still binds only to the intended local interface and Playwright still starts it deterministically.
4. `package.json` and `package-lock.json` contain only the intentional toolchain upgrade; no unrelated package churn is accepted.
