// P03b — the envelope builder mirrors the backend contract exactly.
//
// The whitelist and error codes are pinned against the Rust source, the same
// way `exprInterpreter.test.ts` scans its own file: a command added on one
// side without the other fails here first.

import { describe, expect, it } from 'vitest';
import RUST_COMMANDS from '../../src-tauri/src/runtime/commands.rs?raw';
import {
  COMMAND_ERROR_CODES,
  COMMAND_PROTOCOL_VERSION,
  EVENT_PROTOCOL_VERSION,
  MUTATING_COMMANDS,
  RESEARCH_COMMANDS,
  buildCommandEnvelope,
  isCommandError,
  needsResnapshot,
  newRequestId,
  shouldRetrySameRequest,
} from './researchCommand';

describe('contract pins against the Rust dispatcher', () => {
  it('uses the same protocol versions', () => {
    expect(RUST_COMMANDS).toContain(`pub const COMMAND_PROTOCOL_VERSION: &str = "${COMMAND_PROTOCOL_VERSION}";`);
    expect(RUST_COMMANDS).toContain(`pub const EVENT_PROTOCOL_VERSION: &str = "${EVENT_PROTOCOL_VERSION}";`);
  });

  it('lists exactly the commands the backend whitelists', () => {
    const rustNames = [...RUST_COMMANDS.matchAll(/Command::\w+ => "([a-z]+\.[a-z]+)"/g)].map((m) => m[1]);
    expect(new Set(rustNames).size).toBe(rustNames.length);
    expect([...RESEARCH_COMMANDS].sort()).toEqual([...rustNames].sort());
  });

  it('marks as mutating exactly what the backend tracks by request id', () => {
    const block = /fn mutates\(self\) -> bool \{[\s\S]*?\n {4}\}/.exec(RUST_COMMANDS)?.[0] ?? '';
    const rustMutating = [...block.matchAll(/Command::(\w+)/g)].map((m) => m[1]);
    const toName = (variant: string) => variant.replace(/([a-z])([A-Z])/g, '$1.$2').toLowerCase();
    expect([...MUTATING_COMMANDS].sort()).toEqual(rustMutating.map(toName).sort());
  });

  it('lists exactly the error codes the backend serializes', () => {
    const block = /pub enum ErrorCode \{([\s\S]*?)\}/.exec(RUST_COMMANDS)?.[1] ?? '';
    const rustCodes = block.split('\n').map((line) => line.trim().replace(',', '')).filter(Boolean);
    expect([...COMMAND_ERROR_CODES].sort()).toEqual(rustCodes.sort());
  });
});

describe('buildCommandEnvelope', () => {
  it('builds a complete envelope with a fresh, reusable request id', () => {
    const first = buildCommandEnvelope({ workspaceId: 'ws', command: 'discovery.start', payload: { a: 1 } });
    expect(first.protocolVersion).toBe('research-command-v1');
    expect(first.workspaceId).toBe('ws');
    expect(first.command).toBe('discovery.start');
    expect(first.payload).toEqual({ a: 1 });
    expect(first.requestId).toMatch(/^[0-9a-f-]{36}$/);

    const retry = buildCommandEnvelope({ workspaceId: 'ws', command: 'discovery.start', payload: { a: 1 }, requestId: first.requestId });
    expect(retry).toEqual(first);
    expect(buildCommandEnvelope({ workspaceId: 'ws', command: 'discovery.active' }).payload).toEqual({});
  });

  it('refuses what the backend would refuse, before anything is sent', () => {
    expect(() => buildCommandEnvelope({ workspaceId: '', command: 'discovery.active' })).toThrow(/workspaceId/);
    expect(() => buildCommandEnvelope({ workspaceId: 'ws', command: 'shell.exec' as never })).toThrow(/unknown research command/);
    expect(() => buildCommandEnvelope({ workspaceId: 'ws', command: 'discovery.active', requestId: '  ' })).toThrow(/requestId/);
    expect(() => buildCommandEnvelope({ workspaceId: 'ws', command: 'discovery.active', requestId: 'x'.repeat(129) })).toThrow(/requestId/);
    expect(newRequestId()).not.toBe(newRequestId());
  });
});

describe('error handling helpers', () => {
  it('recognises only the structured error', () => {
    expect(isCommandError({ code: 'Busy', message: 'later', retryable: true })).toBe(true);
    expect(isCommandError({ code: 'Nope', message: 'x', retryable: true })).toBe(false);
    expect(isCommandError('discovery run 1 not found')).toBe(false);
    expect(isCommandError(null)).toBe(false);
  });

  it('retries the same request id only when the backend says so', () => {
    expect(shouldRetrySameRequest({ code: 'Busy', message: 'pending', retryable: true })).toBe(true);
    // A recorded Busy (slot held by another run) comes back final.
    expect(shouldRetrySameRequest({ code: 'Busy', message: 'already paused; this requestId is now final', retryable: false })).toBe(false);
    expect(shouldRetrySameRequest({ code: 'DuplicateRequest', message: '', retryable: true })).toBe(false);
    expect(shouldRetrySameRequest({ code: 'Validation', message: '', retryable: false })).toBe(false);
    expect(shouldRetrySameRequest({ code: 'StaleOwner', message: '', retryable: false })).toBe(false);
  });

  it('asks for a fresh snapshot when the state moved without events or a gap is marked', () => {
    const page = (events: number, stateVersion: number, gap: number | null) =>
      ({ events: Array.from({ length: events }), stateVersion, ledgerGap: gap == null ? null : { stateVersion: gap } });
    expect(needsResnapshot(5, page(0, 5, null))).toBe(false);
    expect(needsResnapshot(5, page(0, 7, null))).toBe(true);
    expect(needsResnapshot(5, page(2, 7, null))).toBe(false);
    expect(needsResnapshot(5, page(2, 7, 6))).toBe(true);
    expect(needsResnapshot(5, page(0, 5, 4))).toBe(false);
    // M1: the full cycle — a gap is found, the reader re-snapshots at that
    // version, and the same page must NOT demand another re-read; only a
    // gap recorded after the new snapshot does.
    expect(needsResnapshot(5, page(0, 7, 7))).toBe(true);
    expect(needsResnapshot(7, page(0, 7, 7))).toBe(false);
    expect(needsResnapshot(7, page(0, 9, 9))).toBe(true);
    expect(needsResnapshot(9, page(0, 9, 9))).toBe(false);
  });
});
