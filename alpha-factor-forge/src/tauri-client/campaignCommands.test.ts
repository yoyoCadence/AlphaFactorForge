// P12d-2d — the typed campaign client is pinned against the Rust commands.
//
// Tauri invoke naming is a listed high-risk area (AGENTS.md): Rust parameters
// are snake_case and arrive under camelCase keys. Every wrapper below is
// called through a mocked `invoke`, and its command name and argument keys
// are compared with the `#[tauri::command]` signature and the handler list,
// so a rename on either side fails here instead of at runtime.

import { beforeEach, describe, expect, it, vi } from 'vitest';
import RUST_CAMPAIGN_COMMANDS from '../../src-tauri/src/commands/campaign_commands.rs?raw';
import RUST_MAIN from '../../src-tauri/src/main.rs?raw';

const tauriMocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriMocks.invoke,
  isTauri: () => true,
}));

import { campaigns } from './commands';

/** Split a parameter list at top-level commas (`State<'_, AppState>` has one). */
function topLevelParams(list: string): string[] {
  const params: string[] = [];
  let depth = 0;
  let current = '';
  for (const char of list) {
    if (char === '<') depth += 1;
    if (char === '>') depth -= 1;
    if (char === ',' && depth === 0) {
      params.push(current);
      current = '';
    } else {
      current += char;
    }
  }
  return [...params, current];
}

/** Parameters the frontend supplies: everything but Tauri-injected state. */
function rustSignatures(): Map<string, string[]> {
  const signatures = new Map<string, string[]>();
  const pattern = /#\[tauri::command\]\s*pub (?:async )?fn (\w+)\s*\(([\s\S]*?)\)\s*->/g;
  for (const match of RUST_CAMPAIGN_COMMANDS.matchAll(pattern)) {
    const params = topLevelParams(match[2])
      .map((param) => param.trim())
      .filter(Boolean)
      .map((param) => param.split(':')[0].trim())
      .filter((name) => name !== 'state' && name !== 'app');
    signatures.set(match[1], params);
  }
  return signatures;
}

const camel = (name: string) => name.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());

const calls: [string, () => Promise<unknown>][] = [
  ['listInstruments', () => campaigns.listInstruments()],
  ['listSnapshots', () => campaigns.listSnapshots()],
  ['preview', () => campaigns.preview({ contractVersion: 'x' })],
  ['freeze', () => campaigns.freeze({ contractVersion: 'x' })],
  ['list', () => campaigns.list()],
  ['start', () => campaigns.start({ envelopeVersion: 'x' }, 'a'.repeat(64), 'crypto:binance:BTCUSDT')],
  ['admission', () => campaigns.admission(7)],
];

describe('campaign commands against the Rust boundary', () => {
  beforeEach(() => {
    tauriMocks.invoke.mockReset();
    tauriMocks.invoke.mockResolvedValue(null);
  });

  it('covers every campaign command the backend defines', () => {
    expect(Object.keys(campaigns).sort()).toEqual(calls.map(([name]) => name).sort());
    expect(rustSignatures().size).toBe(calls.length);
  });

  it.each(calls)('%s invokes a registered command with its exact argument keys', async (_, call) => {
    await call();
    expect(tauriMocks.invoke).toHaveBeenCalledTimes(1);
    const [command, args] = tauriMocks.invoke.mock.calls[0] as [string, Record<string, unknown> | undefined];
    const params = rustSignatures().get(command);
    expect(params, `${command} is a #[tauri::command] in campaign_commands.rs`).toBeDefined();
    expect(Object.keys(args ?? {}).sort()).toEqual(params!.map(camel).sort());
    expect(RUST_MAIN).toContain(`commands::campaign_commands::${command},`);
  });

  it('passes the caller values through unchanged', async () => {
    const config = { envelopeVersion: 'discovery-config-v3' };
    await campaigns.start(config, 'b'.repeat(64), 'crypto:binance:ETHUSDT');
    expect(tauriMocks.invoke).toHaveBeenCalledWith('start_campaign_discovery', {
      config,
      campaignId: 'b'.repeat(64),
      instrumentId: 'crypto:binance:ETHUSDT',
    });
  });
});
