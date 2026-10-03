// P12e-6b — adds runner output to fixtures/research/recalibration-plan-v1-diagnostics.json
// and applies the plan §6 rule. Declarations are never touched.
//
//   npx vite-node scripts/record-recalibration-diagnostics.ts -- size <runner-output.json>
//   npx vite-node scripts/record-recalibration-diagnostics.ts -- power <runner-output.json>
//   npx vite-node scripts/record-recalibration-diagnostics.ts -- required-power <runs.json>
//
// `required-power` writes the runner input for exactly the power runs the
// plan requires, computed from the recorded size reports.
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import {
  pairResults,
  requiredPowerRuns,
  select,
  type DiagnosticReport,
  type DiagnosticRun,
  type Screen,
} from '../src/parity/recalibrationDiagnostics';

const fixturePath = resolve(
  process.cwd(),
  'fixtures/research/recalibration-plan-v1-diagnostics.json',
);

interface Fixture {
  screen: Screen;
  size: (DiagnosticRun & { declaration: Record<string, unknown> })[];
  power: (DiagnosticRun & { declaration: Record<string, unknown> })[];
  reports?: { size: Record<string, DiagnosticReport>; power: Record<string, DiagnosticReport> };
  [key: string]: unknown;
}

const [mode, path] = process.argv.slice(2).filter((argument) => argument !== '--');
const fixture = JSON.parse(await readFile(fixturePath, 'utf8')) as Fixture;
fixture.reports ??= { size: {}, power: {} };

if (mode === 'size' || mode === 'power') {
  const output = JSON.parse(await readFile(path, 'utf8')) as {
    runs: { id: string; report: DiagnosticReport }[];
  };
  const declared = new Set(fixture[mode].map((run) => run.id));
  for (const { id, report } of output.runs) {
    if (!declared.has(id)) throw new Error(`${id} is not a declared ${mode} run`);
    fixture.reports[mode][id] = report;
  }
}

const pairs = pairResults(fixture.size, fixture.reports.size, fixture.screen);
const required = requiredPowerRuns(fixture.power, pairs);
if (mode === 'required-power') {
  const runs = fixture.power
    .filter((run) => required.includes(run.id))
    .map((run) => ({ id: run.id, declaration: run.declaration }));
  await writeFile(path, JSON.stringify({ runs }), 'utf8');
  console.log(`${runs.length} required power runs written to ${path}`);
} else {
  const recordedPower = Object.keys(fixture.reports.power).sort();
  const unexpected = recordedPower.filter((id) => !required.includes(id));
  if (unexpected.length > 0) throw new Error(`power runs outside the plan: ${unexpected.join(', ')}`);
  const complete = required.every((id) => id in fixture.reports!.power);
  fixture.requiredPowerRuns = required;
  if (complete) {
    const { pairs: withPower, selected } = select(pairs, fixture.power, fixture.reports.power);
    fixture.pairs = withPower;
    fixture.selection = selected;
  } else {
    fixture.pairs = pairs;
    delete fixture.selection;
  }
  await writeFile(fixturePath, `${JSON.stringify(fixture, null, 2)}\n`, 'utf8');
  console.log(`recorded ${mode}; ${required.length} power runs required; selection ${complete ? JSON.stringify(fixture.selection) : 'pending'}`);
}
