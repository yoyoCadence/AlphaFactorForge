// P03b — build `research-command-v1` envelopes (docs/research-runtime-contract.md §2).
//
// Pure: no IO. The backend (`runtime::commands::Dispatcher`) is the authority
// on every rule; this module only makes it easy to send a correct envelope
// and hard to send a wrong one:
//   - the protocol version is a constant, never typed by hand;
//   - the command name comes from a whitelist mirrored from the backend;
//   - a request id is minted once per intent and REUSED on retry — that is
//     what makes a retry idempotent instead of a second run.
//
// The UI does not use this yet (the desktop still calls the discovery
// commands directly); P04's bridge / connect mode moves it onto envelopes.

import type {
  CommandEnvelope,
  CommandError,
  CommandErrorCode,
} from '../tauri-client/commands';

export const COMMAND_PROTOCOL_VERSION = 'research-command-v1' as const;
export const EVENT_PROTOCOL_VERSION = 'research-event-v1' as const;

/** The backend whitelist (`runtime::commands::Command::ALL`), `<domain>.<verb>`. */
export const RESEARCH_COMMANDS = [
  'discovery.start',
  'discovery.pause',
  'discovery.resume',
  'discovery.cancel',
  'discovery.progress',
  'discovery.active',
  'events.read',
  'ownership.read',
] as const;

export type ResearchCommand = (typeof RESEARCH_COMMANDS)[number];

/** Commands the backend tracks by request id; the rest are reads. */
export const MUTATING_COMMANDS: readonly ResearchCommand[] = [
  'discovery.start',
  'discovery.pause',
  'discovery.resume',
  'discovery.cancel',
];

export const COMMAND_ERROR_CODES: readonly CommandErrorCode[] = [
  'UnsupportedProtocol',
  'WorkspaceMismatch',
  'Unauthorized',
  'NotOwner',
  'StaleOwner',
  'DuplicateRequest',
  'Validation',
  'NotFound',
  'Busy',
];

const REQUEST_ID_MAX = 128;

/** A fresh request id. Mint one per user intent and keep it for retries. */
export function newRequestId(): string {
  const crypto = globalThis.crypto;
  if (crypto && typeof crypto.randomUUID === 'function') return crypto.randomUUID();
  throw new Error('crypto.randomUUID is unavailable; a request id must be unpredictable');
}

export interface BuildCommandArgs {
  workspaceId: string;
  command: ResearchCommand;
  payload?: unknown;
  /** Reuse the id of the attempt being retried; omit to mint a new one. */
  requestId?: string;
}

/** Build an envelope the backend will accept, or throw before anything is sent. */
export function buildCommandEnvelope(args: BuildCommandArgs): CommandEnvelope {
  if (typeof args.workspaceId !== 'string' || args.workspaceId.trim() === '') {
    throw new RangeError('workspaceId is required; read it from runtime.info() first');
  }
  if (!RESEARCH_COMMANDS.includes(args.command)) {
    throw new RangeError(`unknown research command ${JSON.stringify(args.command)}`);
  }
  const requestId = args.requestId ?? newRequestId();
  if (requestId.trim() === '' || requestId.length > REQUEST_ID_MAX) {
    throw new RangeError(`requestId must be 1–${REQUEST_ID_MAX} characters`);
  }
  return {
    protocolVersion: COMMAND_PROTOCOL_VERSION,
    workspaceId: args.workspaceId,
    requestId,
    command: args.command,
    payload: args.payload ?? {},
  };
}

/** Narrow a rejection from `runtime.dispatch` to the structured error. */
export function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.code === 'string'
    && (COMMAND_ERROR_CODES as readonly string[]).includes(v.code)
    && typeof v.message === 'string'
    && typeof v.retryable === 'boolean'
  );
}

/** Whether a failed attempt may be retried WITH THE SAME request id. The
 *  backend sets `retryable` only for a pending request (first attempt still
 *  executing, or it died before recording anything); every recorded failure
 *  — including a `Busy` caused by a run already holding the slot — is final
 *  for that id and replays verbatim, so the caller must mint a new request.
 *  `DuplicateRequest` is excluded defensively: that id belongs to other
 *  content. */
export function shouldRetrySameRequest(error: CommandError): boolean {
  return error.retryable && error.code !== 'DuplicateRequest';
}

/** The reader's reconnect rule (contract §3, P03b R2): after a snapshot at
 *  `snapshotVersion`, an EMPTY events page whose `stateVersion` moved on, or
 *  any page carrying a ledger gap at or after the snapshot, means the ledger
 *  does not hold everything that happened — take a fresh snapshot. */
export function needsResnapshot(snapshotVersion: number, page: { events: unknown[]; stateVersion: number; ledgerGap: { stateVersion: number } | null }): boolean {
  if (page.ledgerGap != null && page.ledgerGap.stateVersion >= snapshotVersion) return true;
  return page.events.length === 0 && page.stateVersion > snapshotVersion;
}
