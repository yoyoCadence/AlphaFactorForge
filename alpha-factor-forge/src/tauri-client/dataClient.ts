// Thin data-boundary seam used by the UI instead of importing the Tauri client
// directly. PRODUCTION / Tauri always uses the real `tauri-client` wrappers.
//
// In Vite DEV only, the URL flag `?mock=1` swaps in an in-memory mock
// (`mockClient`) so browser E2E (Playwright) can exercise the React UI without
// a Tauri backend. The mock branch is guarded by `import.meta.env.DEV`, so it is
// dead-code-eliminated from production builds and can never activate there.

import {
  db as realDb,
  discovery as realDiscovery,
  files as realFiles,
  isTauri as realIsTauri,
} from './commands';
import {
  onDiscoveryDone,
  onDiscoveryProgress,
  onDiscoveryResult,
} from './events';
import { importDataset as realImportDataset } from './dbClient';
import { makeMockClient } from './mockClient';

// RUNNER-UI-001b-2: the discovery commands and their event subscriptions join the
// seam, so the panel imports neither `commands` nor `events` directly and the
// `?mock=1` runner can drive it. The mock emits real `discovery-event-v1`
// payloads through these same functions, so the browser E2E exercises the
// production parsers rather than a convenient shape.
const realDiscoveryEvents = {
  onProgress: onDiscoveryProgress,
  onResult: onDiscoveryResult,
  onDone: onDiscoveryDone,
};

type Client = {
  db: typeof realDb;
  files: typeof realFiles;
  importDataset: typeof realImportDataset;
  isTauri: typeof realIsTauri;
  discovery: typeof realDiscovery;
  discoveryEvents: typeof realDiscoveryEvents;
};

function pick(): Client {
  if (import.meta.env.DEV) {
    const search = (typeof globalThis !== 'undefined' && globalThis.location?.search) || '';
    if (new URLSearchParams(search).has('mock')) {
      return makeMockClient();
    }
  }
  return {
    db: realDb,
    files: realFiles,
    importDataset: realImportDataset,
    isTauri: realIsTauri,
    discovery: realDiscovery,
    discoveryEvents: realDiscoveryEvents,
  };
}

const client = pick();

export const db = client.db;
export const files = client.files;
export const importDataset = client.importDataset;
export const isTauri = client.isTauri;
export const discovery = client.discovery;
export const discoveryEvents = client.discoveryEvents;
