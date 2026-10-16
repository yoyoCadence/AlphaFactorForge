// The packaged app currently has known CSP denials (FONT-OFFLINE-001 and
// locked Tauri's postMessage fallback). Preserve them as evidence regardless
// of how early CDP attaches; never turn unrelated console errors into success.
const STARTUP_COMMANDS = new Set([
  'plugin%3Aevent%7Clisten', 'get_host_status', 'get_datasets',
  'get_strategies', 'init_database',
]);

export function nativeConsoleObserver(errors, observations, surface = 'main') {
  const commands = surface === 'metrics'
    ? new Set(['plugin%3Aevent%7Clisten']) : STARTUP_COMMANDS;
  return message => {
    const text = message.text();
    const deniedIpc = [...commands].some(command => {
      const url = `http://ipc.localhost/${command}`;
      return (text.startsWith(`Connecting to '${url}' violates the following Content Security Policy directive:`)
        || text.startsWith(`Fetch API cannot load ${url}. Refused to connect because it violates the document's Content Security Policy.`));
    });
    const deniedFont = text.startsWith("Loading the stylesheet 'https://fonts.googleapis.com/css2?")
      && text.includes('Content Security Policy') && text.includes('style-src');
    const ipcFallback = text.startsWith('IPC custom protocol failed, Tauri will now use the postMessage interface instead');
    if (deniedIpc || deniedFont || ipcFallback) {
      observations.push({ type: message.type(), text,
        reason: deniedFont ? 'FONT-OFFLINE-001' : 'locked Tauri IPC fallback' });
    } else if (message.type() === 'error') {
      errors.push((surface === 'main' ? '' : `${surface}: `) + text);
    }
  };
}
