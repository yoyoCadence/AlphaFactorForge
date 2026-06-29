// Thin data-boundary seam used by the UI instead of importing the Tauri client
// directly. PRODUCTION / Tauri always uses the real `tauri-client` wrappers.
//
// In Vite DEV only, the URL flag `?mock=1` swaps in an in-memory mock
// (`mockClient`) so browser E2E (Playwright) can exercise the React UI without
// a Tauri backend. The mock branch is guarded by `import.meta.env.DEV`, so it is
// dead-code-eliminated from production builds and can never activate there.

import { db as realDb, isTauri as realIsTauri } from './commands';
import { importDataset as realImportDataset } from './dbClient';
import { makeMockClient } from './mockClient';

type Client = {
  db: typeof realDb;
  importDataset: typeof realImportDataset;
  isTauri: typeof realIsTauri;
};

function pick(): Client {
  if (import.meta.env.DEV) {
    const search = (typeof globalThis !== 'undefined' && globalThis.location?.search) || '';
    if (new URLSearchParams(search).has('mock')) {
      return makeMockClient();
    }
  }
  return { db: realDb, importDataset: realImportDataset, isTauri: realIsTauri };
}

const client = pick();

export const db = client.db;
export const importDataset = client.importDataset;
export const isTauri = client.isTauri;
