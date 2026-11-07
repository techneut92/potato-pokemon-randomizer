import { sampleRoms, type RomInfo } from './data/roms';
import { sections } from './options/defs';
import { OptionEngine } from './options/engine';
import { fromSettings, toSettings } from './options/settings';
import type { Overrides } from './options/types';
import { BackendError, createBackend, type EditableGame, type EditIssue, type Features } from './backend';
import type { BatchReport } from './generated/BatchReport';
import type { PremadeSeed } from './generated/PremadeSeed';
import type { SharedSettings } from './generated/SharedSettings';
import type { StarterChange } from './generated/StarterChange';
import { isDesktop } from './desktop';
import type { Edit } from './generated/Edit';
import type { EditList } from './generated/EditList';
import type { ObjectRef } from './generated/ObjectRef';
import { EditStore } from './edits/store.svelte';
import { baseName, importError } from './edits/files';
import { blockText, issueLinks } from './edits/issues';
import { EDIT_SECTIONS, refId, runSteps, targetOf, type SectionId } from './edits/sections';
import { DEX } from './data/dex';
import { generationFilters, speciesEntries, type PickRequest } from './picker';
import { NAME_LISTS, namesFromTexts, parseSeed, settingsRomName } from './shared';

export { NAME_LISTS, namesFromTexts, parseSeed, settingsRomName };

/** How long a run takes before its progress dialog opens. */
export const RUN_DIALOG_DELAY_MS = 300;

export type ModalName = 'limit' | 'preset' | 'batch' | 'string' | 'names' | 'theme' | 'update' | 'run' | 'done' | 'pick' | 'importEdits' | 'output' | null;
export type Theme = 'dark' | 'light';
/** The two views of a loaded game. */
export type Mode = 'options' | 'edits';
/** Where the manual editor's data for the loaded game is. */
export type EditStatus = 'idle' | 'loading' | 'failed' | 'ready';

const engine = new OptionEngine(sections());
const backend = createBackend();

/** A ROM on the welcome screen's recent list. */
export interface RecentRom {
  path: string;
  name: string;
  gen: string;
}

const RECENTS_KEY = 'ppr.recentRoms';
const MAX_RECENTS = 6;

/** Recent ROMs live in the webview's storage, per user; failing storage only loses the list. */
function loadRecents(): RecentRom[] {
  if (!isDesktop) return sampleRoms().map((r) => ({ path: r.path, name: r.name, gen: r.gen }));
  try {
    const list: unknown = JSON.parse(localStorage.getItem(RECENTS_KEY) ?? '[]');
    return Array.isArray(list) ? list.filter((r): r is RecentRom => typeof r?.path === 'string' && typeof r?.name === 'string').slice(0, MAX_RECENTS) : [];
  } catch {
    return [];
  }
}

function saveRecents(list: RecentRom[]) {
  try {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(list));
  } catch {
    // Not fatal: the list just isn't remembered.
  }
}

const UPDATES_KEY = 'ppr.gameUpdates';

/** The game update loaded for each game, by its code; loaded again when the game is opened. Per user. */
function loadUpdates(): Record<string, string> {
  if (!isDesktop) return {};
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(UPDATES_KEY) ?? '{}');
    return saved && typeof saved === 'object' ? Object.fromEntries(Object.entries(saved).filter((e): e is [string, string] => typeof e[1] === 'string')) : {};
  } catch {
    return {};
  }
}

function saveUpdates(updates: Record<string, string>) {
  if (!isDesktop) return;
  try {
    localStorage.setItem(UPDATES_KEY, JSON.stringify(updates));
  } catch {
    // Not fatal: the update just isn't loaded again next time.
  }
}

/** A game update laid over the open game. */
export interface LoadedUpdate {
  path: string;
  /** What the game's title screen shows with it, e.g. `Ver. 1.5`. */
  version: string;
}

/** Batch randomization settings. */
export interface BatchOptions {
  on: boolean;
  start: number;
  n: number;
  prefix: string;
  /** Continue numbering where the last batch stopped. */
  adv: boolean;
  logs: boolean;
  logEnding: string;
  /** The output folder; asked for when empty. */
  dir: string;
}

const BATCH_KEY = 'ppr.batch';
const BATCH_DEFAULTS: BatchOptions = { on: false, start: 0, n: 10, prefix: 'random', adv: true, logs: false, logEnding: 'log', dir: '' };

/** Batch settings are remembered per user. */
function loadBatch(): BatchOptions {
  if (!isDesktop) return { ...BATCH_DEFAULTS };
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(BATCH_KEY) ?? '{}');
    return { ...BATCH_DEFAULTS, ...(saved && typeof saved === 'object' ? saved : {}) };
  } catch {
    return { ...BATCH_DEFAULTS };
  }
}


class AppState {
  screen = $state<'welcome' | 'main'>('welcome');
  /** The open game; a sample until the user opens one. */
  rom = $state.raw<RomInfo>(sampleRoms()[1]);
  recent = $state<RecentRom[]>(loadRecents());
  /** What the backend really does; until it has loaded, nothing. */
  features = $state<Features>({ settingsFiles: false, settingsString: false, seedFiles: false, batch: false, gameUpdates: false, log: false, edits: false });
  steps = $state<string[]>([]);
  busy = $state(false);
  /** A randomization is running; its progress dialog shows once it has taken a while. */
  running = $state(false);
  #runDialog: ReturnType<typeof setTimeout> | undefined;
  sec = $state('general');
  /** The category drawer, on windows too narrow for the sidebar. */
  navOpen = $state(false);
  query = $state('');
  ov = $state<Overrides>({});
  theme = $state<Theme>('dark');
  accent = $state('#FF5A5F');
  keepLoaded = $state(false);
  modal = $state<ModalName>(null);
  toast = $state<{ text: string; kind: 'ok' | 'err' } | null>(null);
  /** Limit Pokémon: the generations to include (all by default), and whether their evolutionary relatives come too. */
  inc = $state([1, 1, 1, 1, 1, 1, 1]);
  related = $state(true);
  batch = $state<BatchOptions>(loadBatch());
  /** The Custom Names Editor's texts, one per list; loaded from the backend. */
  names = $state<string[]>(NAME_LISTS.map(() => ''));
  update = $state<LoadedUpdate | null>(null);
  /** What the picker dialog chooses. */
  pick = $state.raw<PickRequest | null>(null);
  /** Randomizer options or manual edits. */
  mode = $state<Mode>('options');
  /** The manual edits of each game opened in this session; `edits.rom` is the loaded one. */
  edits = new EditStore(() => void this.validateEdits());
  /** The manual editor's data for the loaded game, once it has loaded. */
  editView = $state.raw<EditableGame | null>(null);
  editStatus = $state<EditStatus>('idle');
  /** What the game's rules object to in the loaded game's edits; saving is blocked while any of it is an error. */
  editIssues = $state.raw<EditIssue[]>([]);
  /** The manual editor's open section, the object it last jumped to, and whether the Changes list shows. */
  editSec = $state<SectionId>('starters');
  editFocus = $state.raw<ObjectRef | null>(null);
  /** The object each list section shows, by the number in its reference (species number, trainer number, place, …). */
  editSel = $state<Partial<Record<SectionId, number>>>({});
  /** Answers the `output` dialog: whether a 3DS game is saved as a LayeredFS folder, `null` when they cancel. */
  #outputChoice: ((layeredFs: boolean | null) => void) | null = null;
  /** A loaded edits file waiting for the user to let it replace the loaded game's edits (the `importEdits` dialog). */
  pendingImport = $state.raw<{ file: string; list: EditList } | null>(null);
  /** Beside the editor on wide windows (open at first); a drawer below 1440 px (closed at first). */
  changesOpen = $state(typeof matchMedia === 'undefined' || matchMedia('(min-width: 1440px)').matches);
  /** The steps of the running randomization, with applying the manual edits when there are any. */
  runSteps = $state<string[]>([]);
  run = $state({ percent: 0, step: '', rom: 0, count: 0, batch: null as (BatchReport & { dir: string }) | null, seed: '', output: '', layeredFs: false, settingsString: '', shared: null as SharedSettings | null, saveLog: false, starters: [] as StarterChange[], starterHeldItems: [] as StarterChange[], species: 0, baseStatsChanged: 0, typesChanged: 0, abilitiesChanged: 0, expCurvesChanged: 0, evolutionsChanged: 0, typePairs: 0, typePairsChanged: 0, moves: 0, movesChanged: 0, movesetsChanged: 0, tms: 0, tmsChanged: 0, tmHmCompatChanged: 0, tutors: 0, tutorsChanged: 0, tutorCompatChanged: 0, wildSets: 0, wildSetsChanged: 0, statics: 0, staticsChanged: 0, totems: 0, totemsChanged: 0, trades: 0, tradesChanged: 0, fieldItems: 0, fieldItemsChanged: 0, shops: 0, shopsChanged: 0, pickupItems: 0, pickupItemsChanged: 0, trainers: 0, trainersChanged: 0, checkValue: 0, manualEdits: 0 });

  view = $derived(engine.view(this.rom, this.ov, this.query));
  race = $derived(!!this.view.val['gen.race.on']);
  limitOn = $derived(!!this.view.val['gen.limit.on']);
  limitText = $derived.by(() => {
    const gens = this.inc.flatMap((v, i) => (v && i < (this.rom?.genNum ?? 7) ? [i + 1] : []));
    return (gens.length ? 'Gen ' + gens.join(', ') : 'None') + ' · Choose…';
  });
  #toastTimer: ReturnType<typeof setTimeout> | undefined;
  #abort: AbortController | null = null;
  /** The editor's data per ROM path, read once per session. */
  #editViews = new Map<string, EditableGame>();
  /** The latest validation; older answers are dropped. */
  #validation: Promise<void> = Promise.resolve();
  #validationRun = 0;
  /** Why saving is blocked by manual edits, or `''`. */
  saveBlock = $derived(blockText(this.editIssues));

  constructor() {
    void backend.then(async (b) => {
      this.features = b.features;
      this.steps = await b.steps();
      const names = await b.customNames().catch(() => null);
      if (names) this.names = NAME_LISTS.map(([, key]) => names[key].join('\n'));
      await b.onDrop((path) => void this.openPath(path));
    });
  }

  set(key: string, value: unknown) {
    this.ov = { ...this.ov, [key]: value };
  }
  say(text: string, kind: 'ok' | 'err' = 'ok') {
    clearTimeout(this.#toastTimer);
    this.toast = { text, kind };
    this.#toastTimer = setTimeout(() => (this.toast = null), 3600);
  }
  /** Says that a feature isn't built yet, instead of pretending it worked. */
  notYet(feature: string) {
    this.say(feature + ' isn’t available yet.', 'err');
  }
  /** Asks for a ROM and opens it. */
  async openRom() {
    const path = await (await backend).pickRom();
    if (path) await this.openPath(path);
  }
  async openPath(path: string) {
    if (this.busy || this.running) return;
    this.busy = true;
    try {
      const b = await backend;
      const rom = await b.openRom(path);
      this.#show(rom);
      const limited = rom.ported && !rom.ported.groups.length;
      const saved = rom.is3ds && !limited && this.features.gameUpdates ? loadUpdates()[rom.code] : undefined;
      if (saved) {
        try {
          this.#setUpdate(await b.openRom(path, saved), saved);
          return this.say(`Loaded ${rom.name} with its game update (${this.update?.version}).`);
        } catch (e) {
          this.#forgetUpdate(rom.code);
          return this.say(`Loaded ${rom.name}, but not its game update: ${e instanceof BackendError ? e.message : String(e)}. Load the update again.`, 'err');
        }
      }
      this.say(limited ? `Loaded ${rom.name}. Randomizing ${rom.gen.replace('G', 'Gen ')} games isn’t available yet.` : 'Loaded ' + rom.name + '.', limited ? 'err' : 'ok');
    } catch (e) {
      this.say(e instanceof BackendError ? e.message : 'Could not open this file.', 'err');
    } finally {
      this.busy = false;
    }
  }
  /** Makes `rom` the open game, with default options. */
  #show(rom: RomInfo) {
    this.rom = rom;
    this.screen = 'main';
    this.sec = rom.ported?.groups.includes('sp') ? 'starters' : 'general';
    this.ov = {};
    this.update = null;
    this.mode = 'options';
    this.editSec = 'starters';
    this.editFocus = null;
    this.editSel = {};
    this.edits.rom = rom.path;
    this.editView = this.#editViews.get(rom.path) ?? null;
    this.editStatus = this.editView ? 'ready' : 'idle';
    void this.validateEdits();
    this.remember({ path: rom.path, name: rom.name, gen: rom.gen });
  }
  remember(entry: RecentRom) {
    if (!isDesktop) return;
    this.recent = [entry, ...this.recent.filter((r) => r.path !== entry.path)].slice(0, MAX_RECENTS);
    saveRecents(this.recent);
  }
  goTo(section: string) {
    this.navOpen = false;
    this.sec = section;
    this.query = '';
  }
  /** Asks for a game update file; `null` when they cancel. */
  async pickUpdate() {
    return (await backend).pickUpdate();
  }
  /** Lays the update at `path` over the open game and remembers it for the game; false when it can't be loaded. */
  async loadUpdate(path: string) {
    if (this.busy || this.running) return false;
    this.busy = true;
    try {
      this.#setUpdate(await (await backend).openRom(this.rom.path, path), path);
      saveUpdates({ ...loadUpdates(), [this.rom.code]: path });
      this.say('Update saved for game: ' + this.rom.name + '. Output will be a LayeredFS folder.');
      return true;
    } catch (e) {
      this.say(e instanceof BackendError ? e.message : 'Could not load this game update.', 'err');
      return false;
    } finally {
      this.busy = false;
    }
  }
  #setUpdate(rom: RomInfo, path: string) {
    this.rom = rom;
    this.update = { path, version: rom.updateVersion ?? '' };
    this.#forgetEditView();
  }
  /** The game's data changes with its update: the manual editor reads it again, and the edits are checked again. */
  #forgetEditView() {
    this.#editViews.delete(this.rom.path);
    this.editView = null;
    this.editStatus = 'idle';
    if (this.mode === 'edits') void this.loadEditView();
    void this.validateEdits();
  }
  #forgetUpdate(code: string) {
    saveUpdates(Object.fromEntries(Object.entries(loadUpdates()).filter(([c]) => c !== code)));
  }
  /** Removes the game update from the open game, also for the next time it is opened. */
  unloadUpdate() {
    this.#forgetUpdate(this.rom.code);
    this.update = null;
    this.rom = { ...this.rom, updateVersion: undefined };
    this.#forgetEditView();
    this.say('Game update unloaded.');
  }
  closeModal() {
    if (this.modal === 'batch') this.saveBatch();
    if (this.modal === 'output') this.chooseOutput(null);
    this.pendingImport = null;
    if (!this.running) this.modal = null;
  }
  closeRom() {
    this.screen = 'welcome';
    this.query = '';
  }
  resetSection(keys: string[]) {
    const n: Overrides = {};
    for (const [k, v] of Object.entries(this.ov)) if (!keys.some((g) => k === g || k.startsWith(g + '.'))) n[k] = v;
    this.ov = n;
  }
  /** Opens the picker dialog. */
  openPicker(req: PickRequest) {
    this.pick = req;
    this.modal = 'pick';
  }
  /** The species picker of a custom starter option, with Random first. */
  openSpecies(key: string, label: string) {
    const current = String(this.view.val[key + '.v'] ?? '');
    this.openPicker({
      title: 'Choose ' + label,
      lede: 'Every Pokémon in ' + this.rom.name + ' can be a starter. Choose Random to let the randomizer fill this slot.',
      hint: 'Search by name or number, e.g. eevee or 133',
      noun: 'Pokémon',
      entries: speciesEntries(DEX.slice(0, this.rom.dexSize), true),
      specials: [{ value: 'Random', label: 'Random', num: '???' }],
      filters: generationFilters(this.rom.genNum),
      current: current.toLowerCase() === 'random' ? 'Random' : current,
      apply: (v) => this.set(key + '.v', v)
    });
  }

  // ---- Manual edits ----

  /** Switches to the manual editor, reading the game's data first when it hasn't been. */
  showEdits() {
    this.mode = 'edits';
    this.navOpen = false;
    if (this.editStatus === 'idle' || this.editStatus === 'failed') void this.loadEditView();
  }
  showOptions() {
    this.mode = 'options';
    this.navOpen = false;
  }
  /** Reads the editor's data for the loaded game. */
  async loadEditView() {
    const rom = this.rom;
    this.editStatus = 'loading';
    try {
      const view = await (await backend).editableGame(rom);
      this.#editViews.set(rom.path, view);
      if (this.rom.path !== rom.path) return;
      this.editView = view;
      this.editStatus = 'ready';
      void this.validateEdits();
    } catch {
      if (this.rom.path === rom.path) this.editStatus = 'failed';
    }
  }
  /** Opens an editor section, or the object `target` in its section. */
  goToEdit(section: SectionId, target: ObjectRef | null = null) {
    this.mode = 'edits';
    this.navOpen = false;
    this.editSec = section;
    this.editFocus = target;
    if (target) this.editSel = { ...this.editSel, [section]: refId(target) };
  }
  /** Shows object `id` (see `editSel`) in a list section. */
  selectEdit(section: SectionId, id: number) {
    this.editSel = { ...this.editSel, [section]: id };
  }
  /** Edits an object; see `EditStore.set`. */
  setEdit(edit: Edit, original?: Edit) {
    this.edits.set(edit, original);
  }
  /** Reverts the edits of the open section. */
  revertEditSection() {
    const sec = EDIT_SECTIONS.find((s) => s.id === this.editSec);
    if (sec) this.edits.revertKinds(sec.kinds);
  }
  /** Hands the loaded game's edits to the backend, which checks them. A newer check makes an older answer moot. */
  validateEdits(): Promise<void> {
    const run = ++this.#validationRun;
    const view = this.editView;
    const rom = this.rom;
    if (!view) {
      this.editIssues = [];
      return (this.#validation = Promise.resolve());
    }
    const list = this.edits.list(view.game);
    return (this.#validation = (async () => {
      let issues: EditIssue[];
      try {
        issues = await (await backend).setEdits(rom, list);
      } catch (e) {
        // Not knowing whether they fit is no reason to write them: block saving and say why.
        issues = [{ target: targetOf(list.edits[0]), field: null, severity: 'error', message: 'The edits couldn’t be checked: ' + (e instanceof BackendError ? e.message : String(e)) }];
      }
      if (run === this.#validationRun) this.editIssues = issues;
    })());
  }
  /** Asks for an edits file and loads it as the loaded game's edits, asking first when the game has edits. */
  async importEdits() {
    const view = this.editView;
    const rom = this.rom;
    if (!view) return;
    let picked: { path: string; list: EditList } | null;
    try {
      picked = await (await backend).importEdits(rom, view.game);
    } catch (e) {
      this.say(importError(e), 'err');
      return;
    }
    if (!picked || this.rom.path !== rom.path) return;
    const file = baseName(picked.path);
    if (this.edits.count) {
      this.pendingImport = { file, list: picked.list };
      this.modal = 'importEdits';
    } else this.#replaceEdits(file, picked.list);
  }
  /** Lets the file the `importEdits` dialog asks about replace the loaded game's edits. */
  confirmImport() {
    const pending = this.pendingImport;
    this.pendingImport = null;
    this.modal = null;
    if (pending) this.#replaceEdits(pending.file, pending.list);
  }
  /** Replaces the loaded game's edits with a file's; validating them again shows what doesn't fit. */
  #replaceEdits(file: string, list: EditList) {
    this.edits.replace(list.edits);
    this.say(`Edits loaded from ${file}.`);
  }
  /** Asks where and saves the loaded game's edits as an edits file. */
  async exportEdits() {
    const view = this.editView;
    if (!view || !this.edits.count) return;
    try {
      const path = await (await backend).exportEdits(this.rom, this.edits.list(view.game));
      if (path) this.say(`Edits saved to ${path}.`);
    } catch {
      this.say('The edits couldn’t be saved. Please try again.', 'err');
    }
  }
  /** Opens the first object with a problem. */
  showFirstIssue() {
    const first = issueLinks(this.editIssues)[0];
    if (first) this.goToEdit(first.section ?? this.editSec, first.target);
  }
  /** The loaded game's edits for a run, once they are checked; `null` when there are none. */
  async #runEdits() {
    await this.#validation;
    return this.edits.count && this.editView ? this.edits.list(this.editView.game) : null;
  }
  async randomize(seed?: string) {
    const inc = this.inc.some((v, i) => v && i < (this.rom?.genNum ?? 7));
    if (this.limitOn && !inc) {
      this.say('You enabled Limit Pokémon but didn’t choose any to allow.', 'err');
      this.modal = 'limit';
      return;
    }
    if (this.view.badStarter) {
      this.say('One of the custom starters isn’t a Pokémon in this game.', 'err');
      this.sec = 'starters';
      return;
    }
    if (this.race && this.batch.on) {
      this.say('Race Mode and Batch Randomization can’t be used together. Disable one of them.', 'err');
      return;
    }
    const settings = { ...toSettings(this.view.val, this.rom.genNum, { inc: this.inc, related: this.related }), customNames: namesFromTexts(this.names) };
    if (this.race && settings.wild.mode === 'Unchanged' && settings.trainers.mode === 'Unchanged') {
      this.say('Race Mode needs random wild Pokémon or random trainer Pokémon.', 'err');
      return;
    }
    if (this.rom.ported && !this.rom.ported.groups.length) {
      this.say(`Randomizing ${this.rom.gen.replace('G', 'Gen ')} games isn’t available yet.`, 'err');
      return;
    }
    const edits = await this.#runEdits();
    if (this.saveBlock) {
      this.say(this.saveBlock, 'err');
      this.showFirstIssue();
      return;
    }
    // A premade seed is one game, whatever the batch settings.
    const batch = this.batch.on && seed === undefined;
    if (batch && !this.features.batch) return this.notYet('Batch randomization');
    const layeredFs = await this.#askOutput();
    if (layeredFs === null) return;
    if (batch) return this.#randomizeBatch(settings, edits, layeredFs);
    const b = await backend;
    const update = this.update?.path;
    const output = await b.pickOutput(this.rom, layeredFs);
    if (!output) return;
    const settingsString = this.features.settingsString ? await b.settingsString({ settings, extras: this.extras() }).catch(() => '') : '';
    this.#abort = new AbortController();
    this.runSteps = runSteps(this.steps, edits?.edits.length ?? 0);
    this.run = { percent: 0, step: this.steps[0] ?? '', rom: 0, count: 0, batch: null, seed: '', output, layeredFs, settingsString: '', shared: { settings, extras: this.extras() }, saveLog: false, starters: [], starterHeldItems: [], species: 0, baseStatsChanged: 0, typesChanged: 0, abilitiesChanged: 0, expCurvesChanged: 0, evolutionsChanged: 0, typePairs: 0, typePairsChanged: 0, moves: 0, movesChanged: 0, movesetsChanged: 0, tms: 0, tmsChanged: 0, tmHmCompatChanged: 0, tutors: 0, tutorsChanged: 0, tutorCompatChanged: 0, wildSets: 0, wildSetsChanged: 0, statics: 0, staticsChanged: 0, totems: 0, totemsChanged: 0, trades: 0, tradesChanged: 0, fieldItems: 0, fieldItemsChanged: 0, shops: 0, shopsChanged: 0, pickupItems: 0, pickupItemsChanged: 0, trainers: 0, trainersChanged: 0, checkValue: 0, manualEdits: 0 };
    this.#startRun();
    try {
      const res = await b.randomize(
        { rom: this.rom, update, layeredFs, output, settings, seed, log: !this.race, edits },
        (p) => {
          this.run.percent = p.percent;
          this.run.step = p.step;
        },
        this.#abort.signal
      );
      this.run.seed = res.seed;
      this.run.settingsString = settingsString;
      this.run.checkValue = res.checkValue;
      this.run.output = res.output;
      this.run.starters = res.starters;
      this.run.starterHeldItems = res.starterHeldItems;
      this.run.wildSets = res.wildSets;
      this.run.wildSetsChanged = res.wildSetsChanged;
      this.run.species = res.species;
      this.run.baseStatsChanged = res.baseStatsChanged;
      this.run.typesChanged = res.typesChanged;
      this.run.abilitiesChanged = res.abilitiesChanged;
      this.run.expCurvesChanged = res.expCurvesChanged;
      this.run.evolutionsChanged = res.evolutionsChanged;
      this.run.moves = res.moves;
      this.run.movesChanged = res.movesChanged;
      this.run.typePairs = res.typePairs;
      this.run.typePairsChanged = res.typePairsChanged;
      this.run.movesetsChanged = res.movesetsChanged;
      this.run.tms = res.tms;
      this.run.tmsChanged = res.tmsChanged;
      this.run.tmHmCompatChanged = res.tmHmCompatChanged;
      this.run.tutors = res.tutors;
      this.run.tutorsChanged = res.tutorsChanged;
      this.run.tutorCompatChanged = res.tutorCompatChanged;
      this.run.statics = res.statics;
      this.run.staticsChanged = res.staticsChanged;
      this.run.totems = res.totems;
      this.run.totemsChanged = res.totemsChanged;
      this.run.trades = res.trades;
      this.run.tradesChanged = res.tradesChanged;
      this.run.fieldItems = res.fieldItems;
      this.run.fieldItemsChanged = res.fieldItemsChanged;
      this.run.shops = res.shops;
      this.run.shopsChanged = res.shopsChanged;
      this.run.pickupItems = res.pickupItems;
      this.run.pickupItemsChanged = res.pickupItemsChanged;
      this.run.trainers = res.trainers;
      this.run.trainersChanged = res.trainersChanged;
      this.#endRun();
      this.modal = 'done';
    } catch (e) {
      this.#endRun();
      this.modal = null;
      if (e instanceof DOMException && e.name === 'AbortError') this.say('Randomization cancelled. Nothing was saved.', 'err');
      else this.say(e instanceof BackendError ? e.message : 'Randomization failed. Nothing was saved.', 'err');
    }
  }
  /**
   * Whether the game is saved as a LayeredFS folder, `null` when they cancel. Gen 6 and Gen 7 games ask, as the
   * original does, unless a game update is loaded: then it is always a LayeredFS folder. Other games are files.
   */
  #askOutput(): Promise<boolean | null> {
    if (this.update) return Promise.resolve(true);
    if (!this.rom.is3ds) return Promise.resolve(false);
    this.#outputChoice?.(null);
    this.modal = 'output';
    return new Promise((resolve) => (this.#outputChoice = resolve));
  }
  /** Answers the `output` dialog: a LayeredFS folder, a ROM file, or `null` to cancel. */
  chooseOutput(layeredFs: boolean | null) {
    const answer = this.#outputChoice;
    this.#outputChoice = null;
    if (this.modal === 'output') this.modal = null;
    answer?.(layeredFs);
  }
  /** What a settings string holds besides the options: the game, as settings strings name it, and Race Mode. */
  extras() {
    return { romName: settingsRomName(this.rom), raceMode: this.race };
  }
  /** The current settings as a settings string and file hold them. */
  shared(): SharedSettings {
    return { settings: toSettings(this.view.val, this.rom.genNum, { inc: this.inc, related: this.related }), extras: this.extras() };
  }
  /** The settings string of the current settings. */
  async settingsString() {
    return (await backend).settingsString(this.shared());
  }
  /** Replaces the options with loaded ones; custom player graphics stay. */
  #apply(shared: SharedSettings, done: string | null) {
    const loaded = fromSettings(shared.settings, this.rom, shared.extras.raceMode);
    const graphics = Object.fromEntries(Object.entries(this.ov).filter(([k]) => k === 'cpg' || k.startsWith('cpg.')));
    this.ov = { ...loaded.ov, ...graphics };
    if (loaded.limit) {
      this.inc = loaded.limit.inc;
      this.related = loaded.limit.related;
    }
    if (loaded.starterUnavailable) this.say('Could not set one of the custom starters from the settings because it does not exist in this generation.', 'err');
    else if (done) this.say(done);
  }
  /** Why settings couldn't be read, as a message for the user. */
  #settingsError(e: unknown, fallback: string) {
    if (!(e instanceof BackendError)) return fallback;
    if (e.code === 'settings_too_new') return 'The settings were made with a newer randomizer version. Try downloading the latest version.';
    if (e.code === 'settings_too_old') return 'The settings were made with an older randomizer version. Load them in UPR FVX 1.6.2 and save them again first.';
    if (e.code === 'settings_invalid') return fallback;
    return e.message;
  }
  /** Loads a settings string; false when it can't be read. */
  async loadSettingsString(text: string) {
    try {
      this.#apply(await (await backend).readSettingsString(text.trim()), 'Settings loaded from string.');
      return true;
    } catch (e) {
      this.say(this.#settingsError(e, 'The settings string you attempted to load is invalid.'), 'err');
      return false;
    }
  }
  /** Asks for a `.rnqs` file and loads it. */
  async loadSettingsFile() {
    const b = await backend;
    const path = await b.pickSettingsFile('open');
    if (!path) return;
    try {
      this.#apply(await b.loadSettingsFile(path), `Settings loaded from ${path.split(/[\\/]/).pop()}.`);
    } catch (e) {
      this.say(this.#settingsError(e, 'Settings file is not valid.'), 'err');
    }
  }
  /** Asks where and saves the current settings as a `.rnqs` file. */
  async saveSettingsFile() {
    const b = await backend;
    const path = await b.pickSettingsFile('save');
    if (!path) return;
    try {
      this.say('Settings saved to ' + (await b.saveSettingsFile(path, this.shared())) + '.');
    } catch {
      this.say('Settings file save failed. Please try again.', 'err');
    }
  }
  /** A premade seed from a seed and a settings string, as typed into the Premade Seed dialog. */
  async premadeFromString(seed: string, text: string): Promise<PremadeSeed | string> {
    try {
      const shared = await (await backend).readSettingsString(text.trim());
      const parsed = parseSeed(seed);
      return parsed === null ? 'The seed is not a whole number.' : { seed: parsed, shared };
    } catch (e) {
      return this.#settingsError(e, 'The settings string you attempted to load is invalid.');
    }
  }
  /** Asks for a `.rndp` file and reads it; a message when it can't be read, `null` when cancelled. */
  async premadeFromFile(): Promise<{ path: string; premade: PremadeSeed } | string | null> {
    const b = await backend;
    const path = await b.pickSettingsFile('open', 'seed');
    if (!path) return null;
    try {
      return { path, premade: await b.loadPremadeSeed(path) };
    } catch (e) {
      if (e instanceof BackendError && e.code === 'open_failed') return 'Could not load seed file.';
      return this.#settingsError(e, 'The seed file did not contain valid settings.');
    }
  }
  /** Asks for the ROM a premade seed needs; a message when it can't be opened, `null` when cancelled. */
  async premadeRom(): Promise<RomInfo | string | null> {
    const b = await backend;
    const path = await b.pickRom();
    if (!path) return null;
    try {
      return await b.openRom(path);
    } catch (e) {
      return e instanceof BackendError ? e.message : 'Could not open this file.';
    }
  }
  /** Opens `rom` with the premade seed's settings and randomizes with its seed. */
  async usePremade(premade: PremadeSeed, rom: RomInfo) {
    if (this.busy || this.running) return;
    this.modal = null;
    this.#show(rom);
    this.#apply(premade.shared, null);
    await this.randomize(premade.seed);
  }
  /** Saves the last run's seed and settings as a `.rndp` file. */
  async saveSeedFile() {
    const shared = this.run.shared;
    if (!shared || !this.run.seed) return;
    const b = await backend;
    const path = await b.pickSettingsFile('save', 'seed');
    if (!path) return;
    try {
      this.say('Seed file saved to ' + (await b.savePremadeSeed(path, { seed: this.run.seed, shared })) + '.');
    } catch (e) {
      this.say('The seed file couldn’t be saved: ' + (e instanceof BackendError ? e.message : String(e)), 'err');
    }
  }
  /** Remembers the batch settings. */
  saveBatch() {
    if (!isDesktop) return;
    try {
      localStorage.setItem(BATCH_KEY, JSON.stringify(this.batch));
    } catch {
      // Not fatal: they just aren't remembered.
    }
  }
  /** Asks for the batch's output folder. */
  async pickBatchDir() {
    const dir = await (await backend).pickDirectory();
    if (dir) {
      this.batch.dir = dir;
      this.saveBatch();
    }
    return dir;
  }
  /** Batch randomization: `n` ROMs, each with a fresh seed, into the batch folder; with `layeredFs` each a folder. */
  async #randomizeBatch(settings: SharedSettings['settings'], edits: EditList | null, layeredFs: boolean) {
    const dir = this.batch.dir || (await this.pickBatchDir());
    if (!dir) return;
    const b = await backend;
    const count = Math.max(1, Math.floor(this.batch.n));
    const start = Math.max(0, Math.floor(this.batch.start));
    const shared = { settings, extras: this.extras() };
    const settingsString = this.features.settingsString ? await b.settingsString(shared).catch(() => '') : '';
    this.#abort = new AbortController();
    this.runSteps = runSteps(this.steps, edits?.edits.length ?? 0);
    this.run = { ...this.run, percent: 0, step: this.steps[0] ?? '', rom: 1, count, batch: null, seed: '', output: dir, layeredFs, settingsString, shared: null, saveLog: false };
    this.#startRun();
    try {
      const report = await b.randomizeBatch(
        { rom: this.rom, update: this.update?.path, layeredFs, dir, prefix: this.batch.prefix, start, count, settings, logEnding: this.batch.logs ? this.batch.logEnding.replace(/^\./, '') || 'log' : null, edits },
        (p) => {
          this.run.percent = p.percent;
          this.run.step = p.step;
          this.run.rom = p.rom ?? this.run.rom;
        },
        this.#abort.signal
      );
      this.#endRun();
      if (!report.roms.length) {
        this.modal = null;
        this.say('Randomization cancelled. Nothing was saved.', 'err');
        return;
      }
      this.run.batch = { ...report, dir };
      this.modal = 'done';
    } catch (e) {
      this.#endRun();
      this.modal = null;
      this.say(e instanceof BackendError ? e.message : 'Batch randomization failed.', 'err');
    }
  }
  /** Saves the Custom Names Editor's texts; false when they couldn't be saved. */
  async saveNames(texts: string[]) {
    try {
      await (await backend).saveCustomNames(namesFromTexts(texts));
      this.names = texts;
      this.say('Custom names saved.');
      return true;
    } catch (e) {
      this.say('The custom names couldn’t be saved: ' + (e instanceof BackendError ? e.message : String(e)), 'err');
      return false;
    }
  }
  /** Marks a run as started; its progress dialog opens after `RUN_DIALOG_DELAY_MS`, so quick runs don't flash it. */
  #startRun() {
    this.running = true;
    clearTimeout(this.#runDialog);
    this.#runDialog = setTimeout(() => {
      if (this.running) this.modal = 'run';
    }, RUN_DIALOG_DELAY_MS);
  }
  #endRun() {
    clearTimeout(this.#runDialog);
    this.running = false;
  }
  cancelRun() {
    this.#abort?.abort();
  }
  async finish() {
    if (this.run.batch && this.batch.adv) {
      this.batch.start = this.run.batch.nextStart;
      this.saveBatch();
    }
    const saveLog = this.run.saveLog && !this.race && this.features.log;
    this.modal = null;
    if (!this.keepLoaded) {
      this.screen = 'welcome';
      this.ov = {};
      this.update = null;
      this.edits.forget(this.rom.path);
    }
    const kept = this.keepLoaded ? 'Game and settings kept loaded.' : 'Game and settings unloaded.';
    if (!saveLog) return this.say(kept);
    try {
      const path = await (await backend).saveLog();
      this.say(`Log saved to ${path}. ${kept}`);
    } catch (e) {
      this.say(`The log couldn’t be saved: ${e instanceof BackendError ? e.message : String(e)}`, 'err');
    }
  }
}

export const app = new AppState();
