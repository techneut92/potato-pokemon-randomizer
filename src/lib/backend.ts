import type { RomInfo } from './data/roms';
import { sampleRoms } from './data/roms';
import { isDesktop } from './desktop';
import type { Settings } from './generated/Settings';
import type { BatchReport } from './generated/BatchReport';
import type { CustomNames } from './generated/CustomNames';
import type { PremadeSeed } from './generated/PremadeSeed';
import type { SharedSettings } from './generated/SharedSettings';
import type { StarterChange } from './generated/StarterChange';
import type { EditGame } from './generated/EditGame';
import type { EditList } from './generated/EditList';
import { mockEditableGame, mockValidate } from './data/editMock';
import { runSteps } from './edits/sections';
import { EDIT_LIST_VERSION } from './edits/store.svelte';
import type { EditableGame, EditIssue } from './edits/types';

export interface RandomizeRequest {
  rom: RomInfo;
  /** A game update to lay over a 3DS game; the result is then a LayeredFS folder. */
  update?: string;
  /** Save a 3DS game as a LayeredFS folder instead of a ROM file; always so with an update. */
  layeredFs: boolean;
  /** Where to write the result, from `pickOutput`; for a LayeredFS folder, the folder it goes into. */
  output: string;
  settings: Settings;
  seed?: string;
  /** Keep a log of the run for `saveLog`; off in Race Mode. */
  log: boolean;
  /** Manual edits, applied after the randomizers; `null` for none. The same list the last `setEdits` kept. */
  edits: EditList | null;
}

/** A batch: `count` randomizations of `rom` into `dir`, named `<prefix><index>.<ext>` from `start` on. */
export interface BatchRequest {
  rom: RomInfo;
  /** A game update to lay over a 3DS game; each game is then a `<prefix><index>` folder with a LayeredFS folder. */
  update?: string;
  /** Save each 3DS game as a `<prefix><index>` folder with a LayeredFS folder; always so with an update. */
  layeredFs: boolean;
  dir: string;
  prefix: string;
  start: number;
  count: number;
  settings: Settings;
  /** The extension of each ROM's log; `null` for no logs. */
  logEnding: string | null;
  /** Manual edits, applied to every ROM after the randomizers; `null` for none. */
  edits: EditList | null;
}

// The manual editor's view of a game and its validation messages; see `edits/types.ts`.
export type { EditableGame, EditIssue, EditSeverity } from './edits/types';

export interface Progress {
  /** 0–100 */
  percent: number;
  step: string;
  /** In a batch: the ROM being made (from 1) and how many there are. */
  rom?: number;
  count?: number;
}

export interface RandomizeResult {
  seed: string;
  /** What was written: the ROM, or the LayeredFS folder. */
  output: string;
  starters: StarterChange[];
  /** The starters' held items (one for all in Gen 3). */
  starterHeldItems: StarterChange[];
  /** Species in the game, and how many got other base stats. */
  species: number;
  baseStatsChanged: number;
  /** How many species got other types. */
  typesChanged: number;
  /** How many species got other abilities. */
  abilitiesChanged: number;
  /** How many species got another EXP curve. */
  expCurvesChanged: number;
  evolutionsChanged: number;
  moves: number;
  movesChanged: number;
  typePairs: number;
  typePairsChanged: number;
  movesetsChanged: number;
  tms: number;
  tmsChanged: number;
  tmHmCompatChanged: number;
  tutors: number;
  tutorsChanged: number;
  tutorCompatChanged: number;
  /** Wild encounter sets in the game, and how many got another species. */
  wildSets: number;
  wildSetsChanged: number;
  /** Static encounters in the game, and how many got another species. */
  statics: number;
  staticsChanged: number;
  /** Totem Pokémon in the game, and how many changed. */
  totems: number;
  totemsChanged: number;
  /** In-game trades in the game, and how many changed. */
  trades: number;
  tradesChanged: number;
  fieldItems: number;
  fieldItemsChanged: number;
  shops: number;
  shopsChanged: number;
  pickupItems: number;
  trainers: number;
  trainersChanged: number;
  pickupItemsChanged: number;
  /** The check value of the randomized game, shown in Race Mode. */
  checkValue: number;
  /** Manual edits applied after the randomizers. */
  manualEdits: number;
}

/** A failure the user should see, with a stable code (see `ApiError` in `ppr-service`). */
export class BackendError extends Error {
  constructor(
    readonly code: string,
    message: string
  ) {
    super(message);
    this.name = 'BackendError';
  }
}

/** Features a backend really has. The UI says "not available yet" instead of pretending the others work. */
export interface Features {
  settingsFiles: boolean;
  settingsString: boolean;
  seedFiles: boolean;
  batch: boolean;
  gameUpdates: boolean;
  log: boolean;
  /** The manual editor: the editable view, validation and runs with edits. */
  edits: boolean;
}

/** Everything the UI needs from the randomizer: `TauriBackend` in the desktop app, `MockBackend` in the browser. */
export interface Backend {
  readonly features: Features;
  /** Labels of the steps a run reports, in order. */
  steps(): Promise<string[]>;
  /** Asks the user for a ROM; `null` when they cancel. */
  pickRom(): Promise<string | null>;
  /**
   * Reads the ROM at `path`, with the game update at `update` laid over it (3DS games). Throws a `BackendError` when
   * it isn't a supported game, or the update is encrypted or for another game.
   */
  openRom(path: string, update?: string): Promise<RomInfo>;
  /** Asks for a game update (`.cia`); `null` when they cancel. */
  pickUpdate(): Promise<string | null>;
  /** Asks where to save the randomized ROM, or with `layeredFs` the folder for its LayeredFS folder; `null` when they cancel. */
  pickOutput(rom: RomInfo, layeredFs?: boolean): Promise<string | null>;
  /** Randomizes; rejects with an `AbortError` once `signal` aborts, and nothing is written then. */
  randomize(req: RandomizeRequest, onProgress: (p: Progress) => void, signal: AbortSignal): Promise<RandomizeResult>;
  /** Randomizes a batch; once `signal` aborts it stops and reports the ROMs written so far (`cancelled`). */
  randomizeBatch(req: BatchRequest, onProgress: (p: Progress) => void, signal: AbortSignal): Promise<BatchReport>;
  /** Asks for a folder; `null` when they cancel. */
  pickDirectory(): Promise<string | null>;
  /** Saves the log of the last run, which asked for one, next to its ROM; resolves to where. */
  saveLog(): Promise<string>;
  /** The settings string of `shared`. */
  settingsString(shared: SharedSettings): Promise<string>;
  /** Reads a settings string; throws a `BackendError` (`settings_invalid`, `settings_too_old`, `settings_too_new`). */
  readSettingsString(text: string): Promise<SharedSettings>;
  /** Asks for a settings (`.rnqs`) or premade seed (`.rndp`) file to open or save as; `null` when they cancel. */
  pickSettingsFile(mode: 'open' | 'save', kind?: 'settings' | 'seed'): Promise<string | null>;
  /** Saves a `.rnqs` settings file (extension added when missing); resolves to where. */
  saveSettingsFile(path: string, shared: SharedSettings): Promise<string>;
  /** Reads a `.rnqs` settings file; throws a `BackendError` like `readSettingsString`. */
  loadSettingsFile(path: string): Promise<SharedSettings>;
  /** Saves a `.rndp` premade seed file (extension added when missing); resolves to where. */
  savePremadeSeed(path: string, premade: PremadeSeed): Promise<string>;
  /** Reads a `.rndp` premade seed file; throws a `BackendError` like `readSettingsString`. */
  loadPremadeSeed(path: string): Promise<PremadeSeed>;
  /** The Custom Names Editor's lists: the saved ones, or the built-in defaults. */
  customNames(): Promise<CustomNames>;
  /** Saves the Custom Names Editor's lists. */
  saveCustomNames(names: CustomNames): Promise<void>;
  /** Calls `handler` with the path of each file dropped on the window. Returns an unsubscribe function. */
  onDrop(handler: (path: string) => void): Promise<() => void>;
  /** Reads what the manual editor shows of `rom`. Throws a `BackendError` when the data can't be read. */
  editableGame(rom: RomInfo): Promise<EditableGame>;
  /** Keeps `edits` as `rom`'s manual edits and checks them against its game's rules; no issues when they fit. */
  setEdits(rom: RomInfo, edits: EditList): Promise<EditIssue[]>;
  /** Asks where to save `list`, `rom`'s manual edits, and writes it as an edits file; resolves to where, `null` when they cancel. */
  exportEdits(rom: RomInfo, list: EditList): Promise<string | null>;
  /**
   * Asks for an edits file and reads it, checked against `game`, the game the editor shows; `null` when they cancel.
   * Throws a `BackendError`: `edits_invalid`, `edits_too_new`, `edits_unsupported_version`, `edits_wrong_game` (its
   * message names both games) or `open_failed`.
   */
  importEdits(rom: RomInfo, game: EditGame): Promise<{ path: string; list: EditList } | null>;
}

export const abortError = () => new DOMException('Cancelled', 'AbortError');

const MOCK_STEPS = ['Pokémon traits', 'Starters, statics & trades', 'Moves & movesets', 'Trainer Pokémon', 'Wild Pokémon', 'TMs, tutors & items', 'Graphics & tweaks', 'Writing ROM'];

/** Drives the UI in a plain browser (`pnpm dev`) with sample games and a simulated run. */
export class MockBackend implements Backend {
  readonly features: Features = { settingsFiles: true, settingsString: true, seedFiles: true, batch: true, gameUpdates: true, log: true, edits: true };

  async steps() {
    return MOCK_STEPS;
  }
  async pickRom() {
    return sampleRoms()[0].path;
  }
  async openRom(path: string, update?: string) {
    const rom = sampleRoms().find((r) => r.path === path);
    if (!rom) throw new BackendError('rom_unsupported', 'This is not a sample ROM.');
    if (update === undefined) return rom;
    if (!rom.is3ds) throw new BackendError('rom_unsupported', 'this game has no game updates');
    if (!update.endsWith('.cia')) throw new BackendError('rom_unsupported', 'this is not an update for this game');
    return { ...rom, updateVersion: 'Ver. 1.2' };
  }
  async pickUpdate() {
    return '[update-file].cia';
  }
  async pickOutput(rom: RomInfo, layeredFs = false) {
    return layeredFs ? '[output folder]' : '[output folder]/[file name]' + rom.ext;
  }
  randomize(req: RandomizeRequest, onProgress: (p: Progress) => void, signal: AbortSignal): Promise<RandomizeResult> {
    const seed = req.seed || String(Math.floor(1e11 + Math.random() * 9e11));
    const steps = runSteps(MOCK_STEPS, req.edits?.edits.length ?? 0);
    return new Promise((resolve, reject) => {
      let percent = 0;
      const timer = setInterval(() => {
        percent = Math.min(100, percent + 3);
        onProgress({ percent, step: steps[Math.min(steps.length - 1, Math.floor(percent / (100 / steps.length)))] });
        if (percent >= 100) {
          clearInterval(timer);
          resolve({ seed, output: req.layeredFs || req.update ? req.output + '/[title ID]' : req.output, starters: [], starterHeldItems: [], species: 0, baseStatsChanged: 0, typesChanged: 0, abilitiesChanged: 0, expCurvesChanged: 0, evolutionsChanged: 0, typePairs: 0, typePairsChanged: 0, moves: 0, movesChanged: 0, movesetsChanged: 0, tms: 0, tmsChanged: 0, tmHmCompatChanged: 0, tutors: 0, tutorsChanged: 0, tutorCompatChanged: 0, wildSets: 0, wildSetsChanged: 0, statics: 0, staticsChanged: 0, totems: 0, totemsChanged: 0, trades: 0, tradesChanged: 0, fieldItems: 0, fieldItemsChanged: 0, shops: 0, shopsChanged: 0, pickupItems: 0, pickupItemsChanged: 0, trainers: 0, trainersChanged: 0, checkValue: 0, manualEdits: req.edits?.edits.length ?? 0 });
        }
      }, 90);
      signal.addEventListener('abort', () => {
        clearInterval(timer);
        reject(abortError());
      });
    });
  }
  randomizeBatch(req: BatchRequest, onProgress: (p: Progress) => void, signal: AbortSignal): Promise<BatchReport> {
    return new Promise((resolve) => {
      const roms: BatchReport['roms'] = [];
      let i = 0;
      const timer = setInterval(() => {
        if (signal.aborted || i >= req.count) {
          clearInterval(timer);
          resolve({ roms, nextStart: req.start + i, cancelled: signal.aborted && i < req.count });
          return;
        }
        const output = `${req.dir}/${req.prefix}${req.start + i}${req.layeredFs || req.update ? '' : req.rom.ext}`;
        roms.push({ output, seed: String(Math.floor(1e11 + Math.random() * 9e11)), log: req.logEnding ? output.replace(/\.[^.]*$/, '.' + req.logEnding) : null });
        i++;
        onProgress({ percent: Math.round((i * 100) / req.count), step: MOCK_STEPS[MOCK_STEPS.length - 1], rom: i, count: req.count });
      }, 300);
    });
  }
  async pickDirectory() {
    return '[output folder]';
  }
  #names: CustomNames = {
    trainerNames: ['Robin', 'Sasha', 'Milo', 'June', 'Theo', 'Nadia', 'Owen', 'Priya', 'Jonas', 'Leila'],
    trainerClasses: ['Collector', 'Hiker', 'Scientist', 'Ranger', 'Artist', 'Chef'],
    doublesTrainerNames: ['Robin & Sasha', 'Milo & June', 'Theo & Nadia'],
    doublesTrainerClasses: ['Twins', 'Partners', 'Rivals'],
    pokemonNicknames: ['Pebble', 'Comet', 'Mochi', 'Biscuit', 'Nova', 'Ziggy']
  };
  async customNames() {
    return this.#names;
  }
  async saveCustomNames(names: CustomNames) {
    this.#names = names;
  }
  async saveLog() {
    return '[output folder]/[file name].log';
  }
  /** Not the real format: the browser preview only needs strings it can read back. */
  async settingsString(shared: SharedSettings) {
    return 'MOCK' + btoa(unescape(encodeURIComponent(JSON.stringify(shared))));
  }
  async readSettingsString(text: string): Promise<SharedSettings> {
    try {
      if (!text.startsWith('MOCK')) throw new Error();
      return JSON.parse(decodeURIComponent(escape(atob(text.slice(4))))) as SharedSettings;
    } catch {
      throw new BackendError('settings_invalid', 'the settings string is invalid');
    }
  }
  async pickSettingsFile(_mode: 'open' | 'save', kind: 'settings' | 'seed' = 'settings') {
    return kind === 'seed' ? '[shared-seed].rndp' : '[settings].rnqs';
  }
  #files = new Map<string, SharedSettings>();
  #seeds = new Map<string, PremadeSeed>();
  async savePremadeSeed(path: string, premade: PremadeSeed) {
    this.#seeds.set(path, premade);
    return path;
  }
  async loadPremadeSeed(path: string) {
    const premade = this.#seeds.get(path);
    if (!premade) throw new BackendError('open_failed', 'could not open ' + path);
    return premade;
  }
  async saveSettingsFile(path: string, shared: SharedSettings) {
    this.#files.set(path, shared);
    return path;
  }
  async loadSettingsFile(path: string) {
    const shared = this.#files.get(path);
    if (!shared) throw new BackendError('open_failed', 'could not open ' + path);
    return shared;
  }
  async onDrop() {
    return () => {};
  }
  /** The sample game's data, after a moment, as reading a real ROM takes one. */
  editableGame(rom: RomInfo): Promise<EditableGame> {
    const game = this.#view(rom);
    return new Promise((resolve) => setTimeout(() => resolve(game), 400));
  }
  #views = new Map<string, EditableGame>();
  #view(rom: RomInfo): EditableGame {
    let view = this.#views.get(rom.path);
    if (!view) this.#views.set(rom.path, (view = mockEditableGame(rom)));
    return view;
  }
  #edits = new Map<string, EditList>();
  /** The checks the game's rules make, as far as the browser preview can tell them. */
  async setEdits(rom: RomInfo, list: EditList): Promise<EditIssue[]> {
    if (list.game.name !== rom.name) throw new BackendError('edits_wrong_game', `These edits are for ${list.game.name}, not ${rom.name}.`);
    this.#edits.set(rom.path, list);
    return mockValidate(this.#view(rom), list);
  }
  /** Edits files by path, as the JSON a file would hold. Both dialogs answer with the same path, so a saved file loads again. */
  #editFiles = new Map<string, string>();
  readonly editsPath = '[edits].ppredits.json';
  async exportEdits(_rom: RomInfo, list: EditList): Promise<string | null> {
    this.#editFiles.set(this.editsPath, JSON.stringify(list, null, 2) + '\n');
    return this.editsPath;
  }
  /** The checks the backend makes on an edits file, as far as the browser preview needs them. */
  async importEdits(_rom: RomInfo, game: EditGame): Promise<{ path: string; list: EditList } | null> {
    const path = this.editsPath;
    const text = this.#editFiles.get(path);
    if (text === undefined) throw new BackendError('open_failed', 'could not open ' + path);
    let list: EditList;
    try {
      list = JSON.parse(text) as EditList;
      if (typeof list.version !== 'number') throw new Error();
    } catch {
      throw new BackendError('edits_invalid', 'this file isn’t a manual edits file');
    }
    if (list.version > EDIT_LIST_VERSION) throw new BackendError('edits_too_new', `this file was saved by a newer version (edits format ${list.version})`);
    if (!list.game || !Array.isArray(list.edits)) throw new BackendError('edits_invalid', 'this file isn’t a manual edits file');
    if (list.version < EDIT_LIST_VERSION) throw new BackendError('edits_unsupported_version', `this file is in edits format ${list.version}`);
    // The sample games' names already start with "Pokémon".
    if (list.game.name !== game.name || list.game.code !== game.code || list.game.version !== game.version) throw new BackendError('edits_wrong_game', `This file is for ${list.game.name}, but ${game.name} is loaded.`);
    return { path, list };
  }
  /** Puts `text` in the edits file the dialogs answer with, e.g. to try a broken or foreign file in a test. */
  writeEditsFile(text: string) {
    this.#editFiles.set(this.editsPath, text);
  }
}

/** The backend for this environment. The Tauri one is loaded lazily, so the browser build never imports it. */
export async function createBackend(): Promise<Backend> {
  if (!isDesktop) return new MockBackend();
  const { TauriBackend } = await import('./tauriBackend');
  return new TauriBackend();
}
