/**
 * The system the app runs on, read once from the webview's user agent.
 * Neither holds outside a browser, as while the app is built.
 */
const agent = typeof navigator === 'undefined' ? '' : navigator.userAgent;

/** macOS: its keys are spelled ⌘ and ⌥, and its window keeps native traffic lights. */
export const mac = agent.includes('Mac');

/** Android: full screen, with no capture window, tray or updater. */
export const android = agent.includes('Android');
