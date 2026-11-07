import { app } from './app.svelte';
import { isDesktop } from './desktop';

/** Where the support button in the header leads. The desktop app opens the same address (`open_support_page`). */
export const SUPPORT_URL = 'https://ko-fi.com/techneut92';

/** Opens the support page in the default browser; when no browser opens, says where to go instead. */
export async function openSupport(): Promise<void> {
  try {
    if (isDesktop) {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('open_support_page');
    } else {
      window.open(SUPPORT_URL, '_blank', 'noopener');
    }
  } catch {
    app.say(`Couldn't open a browser. The Ko-fi page is ${SUPPORT_URL.replace('https://', '')}`, 'err');
  }
}
