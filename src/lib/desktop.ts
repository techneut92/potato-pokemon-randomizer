/** Running inside the Tauri desktop app (not `pnpm dev` in a browser). */
export const isDesktop = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** macOS keeps its native traffic lights over our title bar; elsewhere the app draws its own window buttons. */
export const isMac = typeof navigator !== 'undefined' && /Macintosh|Mac OS X/.test(navigator.userAgent);
