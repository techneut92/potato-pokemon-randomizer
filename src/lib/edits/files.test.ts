// Edits files: their default name, the messages when one can't be loaded, and the mock backend's checks.
import { describe, expect, it } from 'vitest';
import { BackendError, MockBackend } from '../backend';
import { sampleRoms } from '../data/roms';
import type { EditList } from '../generated/EditList';
import { baseName, editsFileName, importError, replaceQuestion } from './files';
import { EDIT_LIST_VERSION } from './store.svelte';

const emerald = { name: 'Pokémon Emerald (U)', code: 'BPEE', version: null };
const list = (game = emerald, version = EDIT_LIST_VERSION): EditList => ({ version, game, edits: [{ kind: 'starter', slot: 0, species: 1 }] });

describe('edits files', () => {
  it('are named after the game by default, without characters a file name can’t hold', () => {
    expect(editsFileName('Pokémon Emerald (U)')).toBe('Pokémon Emerald (U).ppredits.json');
    expect(editsFileName('Pokémon X/Y: "Demo"')).toBe('Pokémon XY Demo.ppredits.json');
    expect(editsFileName('?')).toBe('Manual edits.ppredits.json');
  });

  it('are shown by their file name', () => {
    expect(baseName('/games/emerald.ppredits.json')).toBe('emerald.ppredits.json');
    expect(baseName('C:\\games\\emerald.ppredits.json')).toBe('emerald.ppredits.json');
  });

  it('ask before replacing the edits a game has', () => {
    expect(replaceQuestion('Pokémon Emerald (U)', 1, 'a.ppredits.json', 5)).toBe('Pokémon Emerald (U) has 1 manual edit. Loading a.ppredits.json replaces them with the 5 manual edits it holds.');
    expect(replaceQuestion('Pokémon Emerald (U)', 3, 'a.ppredits.json', 0)).toBe('Pokémon Emerald (U) has 3 manual edits. Loading a.ppredits.json removes them, as it holds none.');
  });
});

describe('importError', () => {
  const say = (code: string, message = 'from the backend') => importError(new BackendError(code, message));

  it('passes on the backend’s sentence naming both games', () => {
    expect(say('edits_wrong_game', 'This file is for Pokémon Ruby (U), but Pokémon Emerald (U) is loaded.')).toBe('This file is for Pokémon Ruby (U), but Pokémon Emerald (U) is loaded.');
  });

  it('says what is wrong with the file', () => {
    expect(say('edits_too_new')).toBe('These edits were saved by a newer randomizer version. Try downloading the latest version.');
    expect(say('edits_unsupported_version')).toBe('These edits are in an older format this version can’t read.');
    expect(say('edits_invalid')).toBe('This file isn’t a manual edits file, or it is damaged.');
    expect(say('open_failed')).toBe('The edits file couldn’t be opened.');
    expect(say('internal', 'boom')).toBe('boom');
    expect(importError(new Error('x'))).toBe('The edits file couldn’t be loaded.');
  });
});

describe('MockBackend edits files', () => {
  const rom = sampleRoms()[1];
  async function refused(b: MockBackend): Promise<BackendError> {
    const e = await b.importEdits(rom, emerald).catch((e: unknown) => e);
    expect(e).toBeInstanceOf(BackendError);
    return e as BackendError;
  }

  it('loads a saved list again', async () => {
    const b = new MockBackend();
    const path = await b.exportEdits(rom, list());
    expect(path).toBe('[edits].ppredits.json');
    expect(await b.importEdits(rom, emerald)).toEqual({ path, list: list() });
  });

  it('refuses no file, another game, another format and something else', async () => {
    const b = new MockBackend();
    expect((await refused(b)).code).toBe('open_failed');
    await b.exportEdits(rom, list({ name: 'Pokémon Ruby (U)', code: 'AXVE', version: null }));
    expect(await refused(b)).toMatchObject({ code: 'edits_wrong_game', message: 'This file is for Pokémon Ruby (U), but Pokémon Emerald (U) is loaded.' });
    b.writeEditsFile(JSON.stringify({ version: EDIT_LIST_VERSION + 1, game: emerald, edits: [{ kind: 'evolution' }] }));
    expect((await refused(b)).code).toBe('edits_too_new');
    b.writeEditsFile(JSON.stringify(list(emerald, 0)));
    expect((await refused(b)).code).toBe('edits_unsupported_version');
    for (const text of ['not json', '[1]', '{"version": 1}']) {
      b.writeEditsFile(text);
      expect((await refused(b)).code).toBe('edits_invalid');
    }
  });
});
