// SKELETON — minimal frontend shell that BOOTS in Tauri and verifies the
// command bridge (DB init + dataset list). The full AlphaFactorForge research
// workspace UI (Dashboard / Chart / Builder / Discovery / Results / AI Lab /
// Data Manager / Settings) is ported in later — see TODO.md "UI port".

import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { db, isTauri, type Dataset } from './tauri-client/commands';

// Temporary verification harness for the backtest_summary bridge (PR review).
// FKs are ON, so it first seeds a dataset + strategy (both upsert on a fixed
// hash, so re-runs reuse the same parent rows), then saves a summary, reads it
// back, saves again with the SAME key but different metrics, and confirms the
// row was updated in place (not duplicated). Remove during the UI port.
async function runBacktestRoundTrip(log: (line: string) => void): Promise<void> {
  const dsId = await db.importCandles(
    {
      exchange: 'selftest',
      symbol: 'TESTUSDT',
      interval: '1h',
      start_time: 0,
      end_time: 3_600_000,
      candle_count: 2,
      source: 'import',
      dataset_hash: 'selftest-dataset',
    },
    [
      { timestamp: 0, open: 1, high: 2, low: 0.5, close: 1.5, volume: 10 },
      { timestamp: 3_600_000, open: 1.5, high: 2.5, low: 1, close: 2, volume: 12 },
    ],
  );
  log(`dataset_id = ${dsId}`);

  const stratId = await db.saveStrategy({
    name: 'Self-test strategy',
    type: 'params',
    original_definition_json: '{}',
    source: 'manual',
    strategy_hash: 'selftest-strategy',
    lifecycle: 'candidate',
  });
  log(`strategy_id = ${stratId}`);

  const base = { strategy_id: stratId, dataset_id: dsId, segment: 'full' as const, start_time: 0, end_time: 3_600_000 };

  await db.saveBacktestResult({ ...base, net_return: 0.5, cagr: 0.1, trade_count: 7, sharpe: 1.23 });
  const after1 = (await db.getBacktestResults(stratId)).filter((r) => r.segment === 'full');
  log(`save #1 → rows(full)=${after1.length}, net_return=${after1[0]?.net_return}`);

  await db.saveBacktestResult({ ...base, net_return: 0.99, cagr: 0.2, trade_count: 9, sharpe: 4.56 });
  const after2 = (await db.getBacktestResults(stratId)).filter((r) => r.segment === 'full');
  log(`save #2 (same key) → rows(full)=${after2.length}, net_return=${after2[0]?.net_return}`);

  const upsertOk = after1.length === 1 && after2.length === 1 && after2[0]?.net_return === 0.99;
  log(upsertOk ? 'PASS ✅ round-trip + upsert OK (1 row, value updated in place)' : 'FAIL ❌ see rows above');
}

function App(): React.ReactElement {
  const [status, setStatus] = useState<string>('booting…');
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const [testLog, setTestLog] = useState<string[]>([]);
  const [testing, setTesting] = useState<boolean>(false);

  useEffect(() => {
    if (!isTauri()) {
      setStatus('running OUTSIDE Tauri (browser) — backend commands unavailable');
      return;
    }
    (async () => {
      try {
        setStatus(await db.init());
        setDatasets(await db.getDatasets());
      } catch (e) {
        setErr(String(e));
        setStatus('bridge error');
      }
    })();
  }, []);

  async function onRunTest(): Promise<void> {
    setTesting(true);
    setTestLog([]);
    const lines: string[] = [];
    const log = (line: string) => {
      lines.push(line);
      setTestLog([...lines]);
    };
    try {
      await runBacktestRoundTrip(log);
    } catch (e) {
      log(`ERROR ❌ ${String(e)}`);
    } finally {
      setTesting(false);
    }
  }

  return (
    <div style={{ fontFamily: 'ui-monospace, monospace', padding: 24, color: '#16150f' }}>
      <h1 style={{ fontSize: 18, margin: '0 0 4px' }}>AlphaFactorForge — Automated Indicator Discovery Workstation</h1>
      <p style={{ color: '#6b6862', margin: '0 0 16px' }}>status: {status}</p>
      {err && <pre style={{ color: '#b23b2e', whiteSpace: 'pre-wrap' }}>{err}</pre>}
      <h2 style={{ fontSize: 13 }}>datasets ({datasets.length})</h2>
      <ul>
        {datasets.map((d) => (
          <li key={d.id}>
            {d.symbol} · {d.interval} · {d.candle_count} candles
          </li>
        ))}
        {!datasets.length && <li style={{ color: '#aaa' }}>none yet — import via Data Manager (TODO)</li>}
      </ul>

      <h2 style={{ fontSize: 13, marginTop: 24 }}>backtest persistence self-test</h2>
      <button onClick={onRunTest} disabled={testing || !isTauri()} style={{ fontFamily: 'inherit', padding: '4px 10px' }}>
        {testing ? 'running…' : 'Run save → read round-trip'}
      </button>
      {testLog.length > 0 && (
        <pre style={{ background: '#f4f2ec', padding: 12, marginTop: 8, whiteSpace: 'pre-wrap' }}>{testLog.join('\n')}</pre>
      )}
    </div>
  );
}

const el = document.getElementById('root');
if (el) createRoot(el).render(<App />);
