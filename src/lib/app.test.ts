// The app's state driven like the UI drives it, against the mock backend (no Tauri): open a game, change options,
// randomize, save and load settings, batches.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { app, RUN_DIALOG_DELAY_MS } from './app.svelte';
import { sampleRoms } from './data/roms';
import { BackendError, MockBackend } from './backend';

const emerald = () => sampleRoms()[1];

/** Lets the mock backend's timers run until `done` holds. */
async function until(done: () => boolean) {
  for (let i = 0; i < 1000 && !done(); i++) await vi.advanceTimersByTimeAsync(100);
  expect(done()).toBe(true);
}

beforeEach(async () => {
  vi.useFakeTimers();
  await vi.advanceTimersByTimeAsync(0);
  app.modal = null;
  app.busy = false;
  app.batch = { ...app.batch, on: false, dir: '' };
  app.inc = [1, 1, 1, 1, 1, 1, 1];
  await app.openPath(emerald().path);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('opening a game', () => {
  it('shows it with default options', () => {
    expect(app.ov).toEqual({});
    expect(app.screen).toBe('main');
    expect(app.rom.name).toBe('Pokémon Emerald (U)');
    expect(app.toast).toEqual({ text: 'Loaded Pokémon Emerald (U).', kind: 'ok' });
  });

  it('says why a file did not open and keeps the game', async () => {
    await app.openPath('not-a-rom.bin');
    expect(app.toast).toEqual({ text: 'This is not a sample ROM.', kind: 'err' });
    expect(app.rom.name).toBe('Pokémon Emerald (U)');
  });

  it('starts over with default options', async () => {
    app.set('sp', 2);
    await app.openPath(emerald().path);
    expect(app.ov).toEqual({});
  });
});

describe('randomizing', () => {
  it('runs to the Done dialog with the seed it was given', async () => {
    app.set('sp', 2);
    const run = app.randomize('123456789');
    expect(app.modal).not.toBe('done');
    await until(() => app.modal === 'done');
    await run;
    expect(app.run.seed).toBe('123456789');
    expect(app.run.percent).toBe(100);
    expect(app.run.settingsString).toMatch(/^MOCK/);
    expect(app.run.shared?.settings.starters.mode).toBe('Random');
  });

  it('shows its progress only once it has taken a while', async () => {
    const run = app.randomize();
    await until(() => app.running);
    expect(app.modal).toBeNull();
    await vi.advanceTimersByTimeAsync(RUN_DIALOG_DELAY_MS);
    expect(app.modal === 'run' || app.modal === 'done').toBe(true);
    await until(() => app.modal === 'done');
    await run;
    // The dialog's timer is gone once the run is over.
    await vi.advanceTimersByTimeAsync(RUN_DIALOG_DELAY_MS * 2);
    expect(app.modal).toBe('done');
    expect(app.running).toBe(false);
  });

  it('can be cancelled, and then saves nothing', async () => {
    const run = app.randomize();
    await until(() => app.running);
    app.cancelRun();
    await run;
    expect(app.modal).toBeNull();
    expect(app.toast).toEqual({ text: 'Randomization cancelled. Nothing was saved.', kind: 'err' });
  });

  it('refuses Limit Pokémon without generations', async () => {
    app.set('gen.limit.on', true);
    app.inc = [0, 0, 0, 0, 0, 0, 0];
    await app.randomize();
    expect(app.modal).toBe('limit');
    expect(app.toast?.kind).toBe('err');
  });

  it('refuses Race Mode with batches, and Race Mode without random wild or trainer Pokémon', async () => {
    app.set('gen.race.on', true);
    app.batch.on = true;
    await app.randomize();
    expect(app.toast?.text).toMatch(/can’t be used together/);
    app.batch.on = false;
    await app.randomize();
    expect(app.toast?.text).toBe('Race Mode needs random wild Pokémon or random trainer Pokémon.');
    expect(app.modal).toBeNull();
  });

  it('unloads the game when done, unless it is kept loaded', async () => {
    const run = app.randomize('1');
    await until(() => app.modal === 'done');
    await run;
    app.keepLoaded = true;
    await app.finish();
    expect(app.screen).toBe('main');
    app.keepLoaded = false;
    await app.finish();
    expect(app.screen).toBe('welcome');
    expect(app.toast?.text).toBe('Game and settings unloaded.');
  });

  it('saves a log when asked', async () => {
    const run = app.randomize('1');
    await until(() => app.modal === 'done');
    await run;
    app.run.saveLog = true;
    await app.finish();
    expect(app.toast?.text).toMatch(/^Log saved to /);
  });
});

describe('settings', () => {
  it('round-trip through a settings string', async () => {
    app.set('sp', 2);
    app.set('wp', 1);
    const text = await app.settingsString();
    await app.openPath(emerald().path);
    expect(await app.loadSettingsString(text)).toBe(true);
    expect(app.view.val['sp']).toBe(2);
    expect(app.view.val['wp']).toBe(1);
    expect(app.toast?.text).toBe('Settings loaded from string.');
  });

  it('say when a settings string is invalid', async () => {
    expect(await app.loadSettingsString('nonsense')).toBe(false);
    expect(app.toast).toEqual({ text: 'The settings string you attempted to load is invalid.', kind: 'err' });
  });

  it('round-trip through a settings file', async () => {
    app.set('sp', 3);
    await app.saveSettingsFile();
    expect(app.toast?.text).toBe('Settings saved to [settings].rnqs.');
    await app.openPath(emerald().path);
    await app.loadSettingsFile();
    expect(app.view.val['sp']).toBe(3);
  });

  it('keep custom player graphics when others are loaded', async () => {
    const text = await app.settingsString();
    app.set('cpg', 1);
    await app.loadSettingsString(text);
    expect(app.ov['cpg']).toBe(1);
  });
});

describe('premade seeds', () => {
  it('come from a seed and a settings string, and recreate the game', async () => {
    app.set('sp', 2);
    const text = await app.settingsString();
    expect(await app.premadeFromString('12.5', text)).toBe('The seed is not a whole number.');
    const premade = await app.premadeFromString('42', text);
    if (typeof premade === 'string') throw new Error(premade);
    const run = app.usePremade(premade, emerald());
    await until(() => app.modal === 'done');
    await run;
    expect(app.run.seed).toBe('42');
    expect(app.view.val['sp']).toBe(2);
  });

  it('are saved as seed files and read back', async () => {
    const run = app.randomize('77');
    await until(() => app.modal === 'done');
    await run;
    await app.saveSeedFile();
    expect(app.toast?.text).toBe('Seed file saved to [shared-seed].rndp.');
    const loaded = await app.premadeFromFile();
    expect(loaded).toMatchObject({ path: '[shared-seed].rndp', premade: { seed: '77' } });
  });
});

describe('batches', () => {
  it('write the asked number of ROMs and continue the numbering', async () => {
    app.batch = { ...app.batch, on: true, start: 5, n: 3, prefix: 'run', adv: true, logs: true, logEnding: '.txt' };
    const run = app.randomize();
    await until(() => app.modal === 'done');
    await run;
    const report = app.run.batch;
    expect(report?.roms.map((r) => r.output)).toEqual(['[output folder]/run5.gba', '[output folder]/run6.gba', '[output folder]/run7.gba']);
    expect(report?.roms[0].log).toBe('[output folder]/run5.txt');
    await app.finish();
    expect(app.batch.start).toBe(8);
  });

  it('that are cancelled before the first ROM save nothing', async () => {
    app.batch = { ...app.batch, on: true, n: 3 };
    const run = app.randomize();
    await until(() => app.running);
    app.cancelRun();
    await until(() => !app.running);
    await run;
    expect(app.modal).toBeNull();
    expect(app.toast?.text).toBe('Randomization cancelled. Nothing was saved.');
  });

  it('ignore a premade seed', async () => {
    app.batch = { ...app.batch, on: true, n: 3 };
    const run = app.randomize('5');
    await until(() => app.modal === 'done');
    await run;
    expect(app.run.batch).toBeNull();
    expect(app.run.seed).toBe('5');
  });
});

describe('custom names', () => {
  it('are saved from the editor’s texts', async () => {
    const texts = ['Ash\nMisty', 'Hiker', 'A & B', 'Twins', 'Sparky'];
    expect(await app.saveNames(texts)).toBe(true);
    expect(app.names).toEqual(texts);
  });
});

describe('sections', () => {
  it('reset only their own options', () => {
    app.set('sp', 2);
    app.set('sp.held.on', true);
    app.set('wp', 1);
    app.resetSection(['sp']);
    expect(app.ov).toEqual({ wp: 1 });
  });
});

describe('manual edits', () => {
  /** Opens the editor and waits for the game's data. */
  async function openEditor() {
    app.showEdits();
    await until(() => app.editStatus === 'ready');
  }

  beforeEach(async () => {
    app.edits.revertAll();
    await vi.advanceTimersByTimeAsync(0);
  });

  it('open in their own view, after reading the game’s data', async () => {
    expect(app.mode).toBe('options');
    app.showEdits();
    expect(app.mode).toBe('edits');
    expect(app.editStatus).toBe('loading');
    await until(() => app.editStatus === 'ready');
    expect(app.editView?.starters.map((s) => s.species)).toEqual([252, 255, 258]);
    expect(app.editView?.game.code).toBe('BPEE');
    app.showOptions();
    expect(app.mode).toBe('options');
  });

  it('say when the game’s data didn’t load, and can try again', async () => {
    const read = vi.spyOn(MockBackend.prototype, 'editableGame').mockRejectedValueOnce(new Error('broken'));
    await app.openPath(sampleRoms()[0].path);
    app.showEdits();
    await until(() => app.editStatus === 'failed');
    read.mockRestore();
    void app.loadEditView();
    await until(() => app.editStatus === 'ready');
  });

  it('are kept per game while it is open', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    expect(app.edits.count).toBe(1);
    await app.openPath(sampleRoms()[0].path);
    expect(app.mode).toBe('options');
    expect(app.edits.count).toBe(0);
    await app.openPath(emerald().path);
    expect(app.edits.count).toBe(1);
    expect(app.editStatus).toBe('ready');
  });

  it('jump to an object from the Changes list', async () => {
    await openEditor();
    app.showOptions();
    app.goToEdit('starters', { kind: 'starter', slot: 2 });
    expect(app.mode).toBe('edits');
    expect(app.editSec).toBe('starters');
    expect(app.editFocus).toEqual({ kind: 'starter', slot: 2 });
  });

  it('show the object a jump goes to in its section’s list, and keep each section’s choice', async () => {
    await openEditor();
    app.goToEdit('items', { kind: 'itemPrice', item: 12 });
    app.selectEdit('trainers', 7);
    expect(app.editSel).toEqual({ items: 12, trainers: 7 });
    app.goToEdit('shops');
    expect(app.editSel.items).toBe(12);
  });

  it('revert one section only', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    app.setEdit({ kind: 'shop', index: 0, items: [13] });
    app.revertEditSection();
    expect(app.edits.edits).toEqual([{ kind: 'shop', index: 0, items: [13] }]);
  });

  it('are saved to a file and loaded again', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    await app.exportEdits();
    expect(app.toast).toEqual({ text: 'Edits saved to [edits].ppredits.json.', kind: 'ok' });
    app.edits.revertAll();
    await app.importEdits();
    expect(app.modal).toBeNull();
    expect(app.edits.edits).toEqual([{ kind: 'starter', slot: 0, species: 1 }]);
    expect(app.toast).toEqual({ text: 'Edits loaded from [edits].ppredits.json.', kind: 'ok' });
  });

  it('ask before a loaded file replaces the edits the game has', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    await app.exportEdits();
    app.edits.revertAll();
    app.setEdit({ kind: 'shop', index: 0, items: [13] });
    app.setEdit({ kind: 'starter', slot: 2, species: 4 });

    await app.importEdits();
    expect(app.modal).toBe('importEdits');
    expect(app.pendingImport?.file).toBe('[edits].ppredits.json');
    app.closeModal();
    expect(app.pendingImport).toBeNull();
    expect(app.edits.count).toBe(2);

    await app.importEdits();
    app.confirmImport();
    expect(app.modal).toBeNull();
    expect(app.edits.edits).toEqual([{ kind: 'starter', slot: 0, species: 1 }]);
  });

  it('loaded from a file are checked like any others', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 1, species: 999 });
    await app.exportEdits();
    app.edits.revertAll();
    await until(() => !app.saveBlock);
    await app.importEdits();
    await until(() => app.editIssues.length > 0);
    expect(app.saveBlock).toBe('Manual edits: fix 1 problem before saving.');
  });

  it('from another game are refused, naming both games', async () => {
    await app.openPath(sampleRoms()[0].path);
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    await app.exportEdits();
    await app.openPath(emerald().path);
    await openEditor();
    await app.importEdits();
    expect(app.toast).toEqual({ text: 'This file is for Pokémon Platinum (U), but Pokémon Emerald (U) is loaded.', kind: 'err' });
    expect(app.edits.count).toBe(0);
    expect(app.modal).toBeNull();
  });

  it('say why a file can’t be loaded, and keep the edits', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    const load = vi.spyOn(MockBackend.prototype, 'importEdits');
    load.mockRejectedValueOnce(new BackendError('edits_too_new', 'newer'));
    await app.importEdits();
    expect(app.toast).toEqual({ text: 'These edits were saved by a newer randomizer version. Try downloading the latest version.', kind: 'err' });
    load.mockRejectedValueOnce(new BackendError('edits_invalid', 'not edits'));
    await app.importEdits();
    expect(app.toast).toEqual({ text: 'This file isn’t a manual edits file, or it is damaged.', kind: 'err' });
    load.mockResolvedValueOnce(null);
    await app.importEdits();
    expect(app.modal).toBeNull();
    load.mockRestore();
    expect(app.edits.count).toBe(1);
  });

  it('say when they couldn’t be saved', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    const save = vi.spyOn(MockBackend.prototype, 'exportEdits').mockRejectedValueOnce(new BackendError('write_failed', 'disk full'));
    await app.exportEdits();
    save.mockRestore();
    expect(app.toast).toEqual({ text: 'The edits couldn’t be saved. Please try again.', kind: 'err' });
  });

  it('block saving while one breaks a rule, and show where', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 1, species: 999 });
    await until(() => app.editIssues.length > 0);
    expect(app.editIssues).toEqual([{ target: { kind: 'starter', slot: 1 }, field: 'species', severity: 'error', message: 'Starter 2 isn’t a Pokémon in this game.' }]);
    expect(app.saveBlock).toBe('Manual edits: fix 1 problem before saving.');
    app.showOptions();
    const run = vi.spyOn(MockBackend.prototype, 'randomize');
    await app.randomize();
    expect(run).not.toHaveBeenCalled();
    run.mockRestore();
    expect(app.toast).toEqual({ text: 'Manual edits: fix 1 problem before saving.', kind: 'err' });
    expect(app.mode).toBe('edits');
    expect(app.editFocus).toEqual({ kind: 'starter', slot: 1 });
    app.edits.revert({ kind: 'starter', slot: 1 });
    await until(() => !app.saveBlock);
  });

  it('let a warning through', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 1, species: 252 });
    await until(() => app.editIssues.length > 0);
    expect(app.editIssues[0].severity).toBe('warning');
    expect(app.saveBlock).toBe('');
  });

  it('say so when the backend refuses them, and block saving', async () => {
    await openEditor();
    const set = vi.spyOn(MockBackend.prototype, 'setEdits').mockRejectedValueOnce(new BackendError('edits_wrong_game', 'These edits are for Ruby.'));
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    await until(() => app.editIssues.length > 0);
    set.mockRestore();
    expect(app.editIssues).toEqual([{ target: { kind: 'starter', slot: 0 }, field: null, severity: 'error', message: 'The edits couldn’t be checked: These edits are for Ruby.' }]);
    expect(app.saveBlock).not.toBe('');
  });

  it('are sent with the run, which says it applies them', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 }, { kind: 'starter', slot: 0, species: 252 });
    const randomize = vi.spyOn(MockBackend.prototype, 'randomize');
    const steps: string[] = [];
    const run = app.randomize('9');
    await until(() => {
      if (!steps.includes(app.run.step)) steps.push(app.run.step);
      return app.modal === 'done';
    });
    await run;
    expect(randomize.mock.calls[0][0].edits).toEqual({ version: 1, game: { name: 'Pokémon Emerald (U)', code: 'BPEE', version: null }, edits: [{ kind: 'starter', slot: 0, species: 1 }] });
    randomize.mockRestore();
    expect(app.runSteps.at(-2)).toBe('Applying 1 manual edit');
    expect(steps).toContain('Applying 1 manual edit');
  });

  it('are not sent when there are none', async () => {
    const randomize = vi.spyOn(MockBackend.prototype, 'randomize');
    const run = app.randomize('9');
    await until(() => app.modal === 'done');
    await run;
    expect(randomize.mock.calls[0][0].edits).toBeNull();
    randomize.mockRestore();
    expect(app.runSteps).not.toContain('Applying 1 manual edit');
  });

  it('go when the game is unloaded after a run', async () => {
    await openEditor();
    app.setEdit({ kind: 'starter', slot: 0, species: 1 });
    const run = app.randomize('9');
    await until(() => app.modal === 'done');
    await run;
    app.keepLoaded = false;
    await app.finish();
    await app.openPath(emerald().path);
    expect(app.edits.count).toBe(0);
  });
});

describe('game updates', () => {
  const ultraMoon = () => sampleRoms()[2];

  it('lays an update over a 3DS game and saves a LayeredFS folder', async () => {
    await app.openPath(ultraMoon().path);
    expect(await app.loadUpdate('[update-file].cia')).toBe(true);
    expect(app.update).toEqual({ path: '[update-file].cia', version: 'Ver. 1.2' });
    expect(app.toast?.text).toBe('Update saved for game: ' + app.rom.name + '. Output will be a LayeredFS folder.');
    const run = app.randomize('7');
    await until(() => app.modal === 'done');
    await run;
    expect(app.run.output).toBe('[output folder]/[title ID]');
    app.unloadUpdate();
    expect(app.update).toBeNull();
    expect(app.rom.updateVersion).toBeUndefined();
  });

  it('says why an update was refused and keeps the game as it was', async () => {
    expect(await app.loadUpdate('[update-file].cia')).toBe(false);
    expect(app.toast).toEqual({ text: 'this game has no game updates', kind: 'err' });
    expect(app.update).toBeNull();
  });

  it('drops the update with the game', async () => {
    await app.openPath(ultraMoon().path);
    await app.loadUpdate('[update-file].cia');
    await app.openPath(ultraMoon().path);
    expect(app.update).toBeNull();
  });
});

describe('3DS output', () => {
  const ultraMoon = () => sampleRoms()[2];

  /** Starts a run of the open 3DS game and waits for the output question; `run` is the run itself. */
  async function asked(seed?: string) {
    const run = app.randomize(seed);
    await until(() => app.modal === 'output');
    // Wrapped: an async function would wait for a returned promise.
    return { run };
  }

  it('asks for a ROM file or a LayeredFS folder, and saves a LayeredFS folder without an update', async () => {
    await app.openPath(ultraMoon().path);
    const randomize = vi.spyOn(MockBackend.prototype, 'randomize');
    const pickOutput = vi.spyOn(MockBackend.prototype, 'pickOutput');
    const { run } = await asked('3');
    app.chooseOutput(true);
    await until(() => app.modal === 'done');
    await run;
    expect(pickOutput.mock.calls[0][1]).toBe(true);
    expect(randomize.mock.calls[0][0]).toMatchObject({ layeredFs: true, update: undefined, output: '[output folder]' });
    expect(app.run.layeredFs).toBe(true);
    expect(app.run.output).toBe('[output folder]/[title ID]');
    randomize.mockRestore();
    pickOutput.mockRestore();
  });

  it('saves a ROM file when asked to', async () => {
    await app.openPath(ultraMoon().path);
    const randomize = vi.spyOn(MockBackend.prototype, 'randomize');
    const { run } = await asked('3');
    app.chooseOutput(false);
    await until(() => app.modal === 'done');
    await run;
    expect(randomize.mock.calls[0][0].layeredFs).toBe(false);
    expect(app.run.layeredFs).toBe(false);
    expect(app.run.output).toBe('[output folder]/[file name].3ds');
    randomize.mockRestore();
  });

  it('runs nothing when the question is cancelled', async () => {
    await app.openPath(ultraMoon().path);
    const randomize = vi.spyOn(MockBackend.prototype, 'randomize');
    const { run } = await asked();
    app.closeModal();
    await run;
    expect(app.modal).toBeNull();
    expect(app.running).toBe(false);
    expect(randomize).not.toHaveBeenCalled();
    randomize.mockRestore();
  });

  it('does not ask with an update loaded, nor for other games', async () => {
    const randomize = vi.spyOn(MockBackend.prototype, 'randomize');
    let run = app.randomize('1');
    await until(() => app.modal === 'done');
    await run;
    expect(randomize.mock.calls[0][0].layeredFs).toBe(false);
    app.modal = null;
    await app.openPath(ultraMoon().path);
    await app.loadUpdate('[update-file].cia');
    run = app.randomize('1');
    await until(() => app.modal === 'done');
    await run;
    expect(randomize.mock.calls[1][0].layeredFs).toBe(true);
    randomize.mockRestore();
  });

  it('asks before a batch, which then saves folders', async () => {
    await app.openPath(ultraMoon().path);
    app.batch = { ...app.batch, on: true, start: 0, n: 2, prefix: 'run', adv: false, logs: false };
    const batch = vi.spyOn(MockBackend.prototype, 'randomizeBatch');
    const { run } = await asked();
    app.chooseOutput(true);
    await until(() => app.modal === 'done');
    await run;
    expect(batch.mock.calls[0][0].layeredFs).toBe(true);
    expect(app.run.batch?.roms.map((r) => r.output)).toEqual(['[output folder]/run0', '[output folder]/run1']);
    batch.mockRestore();
  });
});
