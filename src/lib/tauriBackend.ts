import { Channel, invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { abortError, BackendError, type Backend, type BatchRequest, type EditableGame, type EditIssue, type Features, type Progress, type RandomizeRequest, type RandomizeResult } from './backend';
import { romInfo } from './data/loaded';
import type { RomInfo } from './data/roms';
import type { ApiError } from './generated/ApiError';
import type { LoadedRom } from './generated/LoadedRom';
import type { RunReport } from './generated/RunReport';
import type { BatchReport } from './generated/BatchReport';
import type { CustomNames } from './generated/CustomNames';
import type { PremadeSeed } from './generated/PremadeSeed';
import type { SharedSettings } from './generated/SharedSettings';
import type { EditGame } from './generated/EditGame';
import type { EditList } from './generated/EditList';
import { editsFileName } from './edits/files';

const ROM_FILTER = { name: 'Pokémon ROMs', extensions: ['gb', 'gbc', 'gba', 'nds', '3ds', 'cci', 'cia', 'cxi'] };
const SETTINGS_FILTER = { name: 'Randomizer settings', extensions: ['rnqs'] };
const SEED_FILTER = { name: 'Premade seeds', extensions: ['rndp'] };
const UPDATE_FILTER = { name: 'Nintendo 3DS game update', extensions: ['cia'] };
// Edits files end in `.ppredits.json`; dialogs filter on the last extension only.
const EDITS_FILTER = { name: 'Manual edits', extensions: ['json'] };

/**
 * File dialogs. The end-to-end test (e2e/smoke.mjs) can't click native dialogs, so it puts its answers in
 * `window.__PPR_DIALOG_ANSWERS__` first; each dialog then takes the next answer instead of opening. Pages can call
 * any command anyway, so this gives them nothing new.
 */
function answered(): string | null | undefined {
  const answers = (window as { __PPR_DIALOG_ANSWERS__?: (string | null)[] }).__PPR_DIALOG_ANSWERS__;
  return Array.isArray(answers) ? (answers.shift() ?? null) : undefined;
}
const open: typeof openDialog = (options) => {
  const a = answered();
  return a === undefined ? openDialog(options) : (Promise.resolve(a) as ReturnType<typeof openDialog>);
};
const save: typeof saveDialog = (options) => {
  const a = answered();
  return a === undefined ? saveDialog(options) : Promise.resolve(a);
};

/** Commands reject with an `ApiError`; anything else is a bug or an IPC failure. */
function toError(e: unknown): BackendError {
  const api = e as Partial<ApiError> | null;
  if (api && typeof api.code === 'string' && typeof api.message === 'string') return new BackendError(api.code, api.message);
  return new BackendError('internal', String(e));
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toError(e);
  }
}

/** `/games/Emerald.gba` → `/games/Emerald-randomized.gba`. */
export function defaultOutput(path: string): string {
  return path.replace(/(\.[^./\\]+)?$/, (ext) => '-randomized' + ext);
}

/** The Rust backend, through Tauri commands (ADR 0004). */
export class TauriBackend implements Backend {
  readonly features: Features = { settingsFiles: true, settingsString: true, seedFiles: true, batch: true, gameUpdates: true, log: true, edits: true };

  steps() {
    return call<string[]>('randomization_steps');
  }

  async pickRom() {
    const path = await open({ multiple: false, directory: false, filters: [ROM_FILTER] });
    return typeof path === 'string' ? path : null;
  }

  async openRom(path: string, update?: string): Promise<RomInfo> {
    return romInfo(await call<LoadedRom>('open_rom', { path, update: update ?? null }), path);
  }

  async pickUpdate() {
    const path = await open({ multiple: false, directory: false, filters: [UPDATE_FILTER] });
    return typeof path === 'string' ? path : null;
  }

  async pickOutput(rom: RomInfo, layeredFs = false) {
    if (layeredFs) return this.pickDirectory();
    const ext = rom.ext.replace('.', '');
    return save({ defaultPath: defaultOutput(rom.path), filters: [{ name: 'ROM', extensions: ext ? [ext] : [] }] });
  }

  async randomize(req: RandomizeRequest, onProgress: (p: Progress) => void, signal: AbortSignal): Promise<RandomizeResult> {
    if (signal.aborted) throw abortError();
    const channel = new Channel<Progress>();
    channel.onmessage = onProgress;
    const cancel = () => void invoke('cancel_randomize');
    signal.addEventListener('abort', cancel);
    try {
      return await call<RunReport>('randomize', { input: req.rom.path, update: req.update ?? null, layeredFs: req.layeredFs, output: req.output, settings: req.settings, seed: req.seed ?? null, log: req.log, edits: req.edits, onProgress: channel });
    } catch (e) {
      if (e instanceof BackendError && e.code === 'cancelled') throw abortError();
      throw e;
    } finally {
      signal.removeEventListener('abort', cancel);
    }
  }

  async randomizeBatch(req: BatchRequest, onProgress: (p: Progress) => void, signal: AbortSignal): Promise<BatchReport> {
    const channel = new Channel<Progress>();
    channel.onmessage = onProgress;
    const cancel = () => void invoke('cancel_randomize');
    if (signal.aborted) return { roms: [], nextStart: req.start, cancelled: true };
    signal.addEventListener('abort', cancel);
    try {
      return await call<BatchReport>('randomize_batch', { input: req.rom.path, update: req.update ?? null, layeredFs: req.layeredFs, dir: req.dir, prefix: req.prefix, start: req.start, count: req.count, settings: req.settings, logEnding: req.logEnding, edits: req.edits, onProgress: channel });
    } finally {
      signal.removeEventListener('abort', cancel);
    }
  }

  customNames() {
    return call<CustomNames>('custom_names');
  }

  async saveCustomNames(names: CustomNames) {
    await call<null>('save_custom_names', { names });
  }

  async pickDirectory() {
    const path = await open({ multiple: false, directory: true });
    return typeof path === 'string' ? path : null;
  }

  saveLog() {
    return call<string>('save_run_log');
  }

  settingsString(shared: SharedSettings) {
    return call<string>('settings_string', { shared });
  }

  readSettingsString(text: string) {
    return call<SharedSettings>('read_settings_string', { text });
  }

  async pickSettingsFile(mode: 'open' | 'save', kind: 'settings' | 'seed' = 'settings') {
    const filters = [kind === 'seed' ? SEED_FILTER : SETTINGS_FILTER];
    const path = mode === 'open' ? await open({ multiple: false, directory: false, filters }) : await save({ filters });
    return typeof path === 'string' ? path : null;
  }

  saveSettingsFile(path: string, shared: SharedSettings) {
    return call<string>('save_settings_file', { path, shared });
  }

  loadSettingsFile(path: string) {
    return call<SharedSettings>('load_settings_file', { path });
  }

  savePremadeSeed(path: string, premade: PremadeSeed) {
    return call<string>('save_premade_seed', { path, premade });
  }

  loadPremadeSeed(path: string) {
    return call<PremadeSeed>('load_premade_seed', { path });
  }

  onDrop(handler: (path: string) => void) {
    return getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === 'drop' && event.payload.paths.length) handler(event.payload.paths[0]);
    });
  }

  editableGame(rom: RomInfo) {
    return call<EditableGame>('editable_game', { path: rom.path });
  }

  setEdits(rom: RomInfo, edits: EditList) {
    return call<EditIssue[]>('set_edits', { path: rom.path, edits });
  }

  async exportEdits(rom: RomInfo, list: EditList) {
    // Next to the ROM, named after the game.
    const defaultPath = rom.path.replace(/[^/\\]*$/, '') + editsFileName(rom.name);
    const path = await save({ defaultPath, filters: [EDITS_FILTER] });
    return path ? call<string>('save_edits_file', { path, list }) : null;
  }

  async importEdits(_rom: RomInfo, game: EditGame): Promise<{ path: string; list: EditList } | null> {
    const path = await open({ multiple: false, directory: false, filters: [EDITS_FILTER] });
    if (typeof path !== 'string') return null;
    return { path, list: await call<EditList>('load_edits_file', { path, game }) };
  }
}
