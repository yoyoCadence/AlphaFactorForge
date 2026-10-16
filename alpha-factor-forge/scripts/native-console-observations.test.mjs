import assert from 'node:assert/strict';
import test from 'node:test';
import { nativeConsoleObserver } from './native-console-observations.mjs';

const deniedConnect = command => `Connecting to 'http://ipc.localhost/${command}' violates the following Content Security Policy directive: "default-src 'self'". Note that 'connect-src' was not explicitly set, so 'default-src' is used as a fallback. The action has been blocked.`;
const deniedFetch = command => `Fetch API cannot load http://ipc.localhost/${command}. Refused to connect because it violates the document's Content Security Policy.`;
const deniedFont = `Loading the stylesheet 'https://fonts.googleapis.com/css2?family=IBM+Plex+Sans&display=swap' violates the following Content Security Policy directive: "style-src 'self' 'unsafe-inline'".`;
const message = (text, type = 'error') => ({ text: () => text, type: () => type });

test('early main attachment retains the observed CI startup denials as evidence', () => {
  const errors = [], observations = [];
  const observe = nativeConsoleObserver(errors, observations);
  const texts = ['plugin%3Aevent%7Clisten', 'get_host_status', 'get_datasets',
    'get_strategies', 'init_database'].flatMap(command => [deniedConnect(command), deniedFetch(command)]);
  texts.push(deniedFont);
  texts.forEach(text => observe(message(text)));
  assert.deepEqual(errors, []);
  assert.deepEqual(observations.map(entry => entry.text), texts);
  assert.equal(observations.at(-1).reason, 'FONT-OFFLINE-001');
});

test('late attachment and metrics attachment keep their own unexpected errors fatal', () => {
  const errors = [], observations = [];
  const observe = nativeConsoleObserver(errors, observations, 'metrics');
  observe(message(deniedConnect('plugin%3Aevent%7Clisten')));
  observe(message(deniedFetch('plugin%3Aevent%7Clisten')));
  observe(message(deniedFont));
  observe(message(deniedConnect('get_datasets')));
  observe(message('Uncaught TypeError: result synchronization failed'));
  assert.equal(observations.length, 3);
  assert.deepEqual(errors, ['metrics: ' + deniedConnect('get_datasets'),
    'metrics: Uncaught TypeError: result synchronization failed']);
});

test('changed origins, commands and other IPC/network failures cannot hide as known CSP observations', () => {
  const errors = [], observations = [];
  const observe = nativeConsoleObserver(errors, observations);
  const texts = [deniedConnect('get_secret'), deniedFetch('get_datasets').replace('http:', 'https:'),
    deniedConnect('get_datasets').replace('ipc.localhost', 'example.com'),
    'Failed to fetch http://ipc.localhost/get_datasets: backend unavailable',
    "Loading the stylesheet 'https://example.com/fonts.css' violates Content Security Policy style-src",
    'TypeError: Cannot read properties of undefined'];
  texts.forEach(text => observe(message(text)));
  assert.deepEqual(errors, texts);
  assert.deepEqual(observations, []);
});

test('the locked fallback warning is retained without discarding an adjacent console error', () => {
  const errors = [], observations = [];
  const observe = nativeConsoleObserver(errors, observations);
  observe(message('IPC custom protocol failed, Tauri will now use the postMessage interface instead', 'warn'));
  observe(message('postMessage invocation failed'));
  assert.equal(observations.length, 1);
  assert.equal(observations[0].type, 'warn');
  assert.deepEqual(errors, ['postMessage invocation failed']);
});
