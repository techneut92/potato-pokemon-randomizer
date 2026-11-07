// What the editor shows about edits: validation messages, the Changes list, run steps and pickers.
import { describe, expect, it } from 'vitest';
import type { EditIssue, EditSeverity } from '../backend';
import { generationFilters, matches, speciesEntries, withOriginal } from '../picker';
import { describeChanges, starterRole } from './changes';
import { fixtureGame } from './fixture';
import { blockText, errorCount, issueLinks, issuesOf, sharedIssues } from './issues';
import { speciesPicker } from './pickers';
import { refKey, runSteps, sectionOf, targetOf } from './sections';

const issue = (target: EditIssue['target'], message: string, severity: EditSeverity = 'error'): EditIssue => ({ target, field: null, severity, message });
const view = fixtureGame();

describe('validation messages', () => {
  const issues = [
    issue({ kind: 'shop', index: 2 }, 'Mart sells nothing.'),
    issue({ kind: 'starter', slot: 1 }, 'Starter 2 isn’t a Pokémon in this game.'),
    issue({ kind: 'trainer', index: 7 }, 'Roxanne has 7 Pokémon.'),
    issue({ kind: 'starter', slot: 1 }, 'Second problem.'),
    issue({ kind: 'starter', slot: 1 }, 'Same as starter 1.', 'warning')
  ];

  it('link to their object, in section order', () => {
    const links = issueLinks(issues);
    expect(links.map((l) => [l.section, l.goLabel, l.text])).toEqual([
      ['starters', 'Open starter', 'Starter 2 isn’t a Pokémon in this game.'],
      ['starters', 'Open starter', 'Second problem.'],
      ['trainers', 'Open trainer', 'Roxanne has 7 Pokémon.'],
      ['shops', 'Open shop', 'Mart sells nothing.']
    ]);
    expect(links[0].target).toEqual({ kind: 'starter', slot: 1 });
    expect(new Set(links.map((l) => l.key)).size).toBe(links.length);
  });

  it('keep warnings apart from errors', () => {
    expect(issueLinks(issues, 'warning').map((l) => [l.text, l.severity])).toEqual([['Same as starter 1.', 'warning']]);
  });

  it('show with their object, errors first', () => {
    const warning = issue({ kind: 'starter', slot: 1 }, 'Same as starter 1.', 'warning');
    expect(issuesOf([warning, ...issues.slice(0, 4)], { kind: 'starter', slot: 1 }).map((i) => i.message)).toEqual(['Starter 2 isn’t a Pokémon in this game.', 'Second problem.', 'Same as starter 1.']);
    expect(issuesOf(issues, { kind: 'starter', slot: 0 })).toEqual([]);
    expect(issuesOf(issues, { kind: 'starterHeldItem', slot: 1 })).toEqual([]);
    expect(errorCount(issues, 'starters')).toBe(2);
    expect(errorCount(issues, 'items')).toBe(0);
  });

  it('link a limit shops share to the shops, and one about the whole list nowhere', () => {
    const space = { target: null, field: 'shop space', severity: 'error' as const, message: 'The shop lists need 6 bytes more.' };
    const list = { target: null, field: null, severity: 'error' as const, message: 'These edits are for another game.' };
    const links = issueLinks([...issues, space, list]);
    expect(links[0]).toMatchObject({ section: null, goLabel: '', text: 'These edits are for another game.' });
    expect(links.find((l) => l.text === space.message)).toMatchObject({ section: 'shops', goLabel: 'Open shops', target: null });
    expect(sharedIssues([...issues, space, list], 'shops')).toEqual([space]);
    expect(errorCount([space], 'shops')).toBe(1);
    expect(issuesOf([space], { kind: 'shop', index: 0 })).toEqual([]);
  });

  it('block saving while there are errors, saying how many', () => {
    expect(blockText([])).toBe('');
    expect(blockText(issues.slice(0, 1))).toBe('Manual edits: fix 1 problem before saving.');
    expect(blockText(issues)).toBe('Manual edits: fix 4 problems before saving.');
    expect(blockText(issues.slice(4))).toBe('');
  });
});

describe('objects and sections', () => {
  it('name each edit’s object like Rust does', () => {
    expect(targetOf({ kind: 'starter', slot: 2, species: 1 })).toEqual({ kind: 'starter', slot: 2 });
    expect(targetOf({ kind: 'itemPrice', item: 13, price: 200 })).toEqual({ kind: 'itemPrice', item: 13 });
    expect(targetOf({ kind: 'fieldItem', index: 4, item: 13 })).toEqual({ kind: 'fieldItem', index: 4 });
    expect(refKey({ kind: 'trainer', index: 7 })).toBe('trainer:7');
  });

  it('put every edit kind in one section', () => {
    expect(sectionOf('starterHeldItem').id).toBe('starters');
    expect(sectionOf('wildSet').id).toBe('wild');
    expect(sectionOf('itemPrice').id).toBe('items');
  });
});

describe('the Changes list', () => {
  it('shows a starter’s before and after', () => {
    expect(describeChanges([{ kind: 'starter', slot: 1, species: 2 }], view)).toEqual([
      { key: 'starter:1', target: { kind: 'starter', slot: 1 }, section: sectionOf('starter'), label: 'Starter 2', detail: 'Charmander → Ivysaur' }
    ]);
  });


  it('calls starters by their role in the game', () => {
    expect(starterRole(0, 2, 1).role).toBe('Your starter');
    expect(starterRole(1, 2, 1).role).toBe('Rival’s starter');
    expect(starterRole(4, 6, 6).role).toBe('Kanto starter 2');
    expect(starterRole(2, 3, 3).role).toBe('Starter 3');
  });
});

describe('run steps', () => {
  const steps = ['Pokémon traits', 'Wild Pokémon', 'Writing ROM'];
  it('apply the edits just before writing the ROM', () => {
    expect(runSteps(steps, 0)).toEqual(steps);
    expect(runSteps(steps, 1)).toEqual(['Pokémon traits', 'Wild Pokémon', 'Applying 1 manual edit', 'Writing ROM']);
    expect(runSteps(steps, 3)[2]).toBe('Applying 3 manual edits');
  });
});

describe('pickers', () => {
  const entries = speciesEntries(['Bulbasaur', 'Ivysaur', 'Venusaur', 'Charmander']);

  it('search by name or number', () => {
    expect(entries.filter((e) => matches(e, 'saur')).map((e) => e.value)).toEqual([1, 2, 3]);
    expect(entries.filter((e) => matches(e, '#004')).map((e) => e.value)).toEqual([4]);
    expect(entries.filter((e) => matches(e, '4')).map((e) => e.value)).toEqual([4]);
    expect(entries.filter((e) => matches(e, '  ')).length).toBe(4);
  });

  it('filter by generation', () => {
    const f = generationFilters(2);
    expect(f.options.map((o) => o.label)).toEqual(['All', 'Gen 1', 'Gen 2']);
    const chikorita = { value: 152, label: 'Chikorita', num: '#152' };
    expect(f.options[1].test(chikorita)).toBe(false);
    expect(f.options[2].test(chikorita)).toBe(true);
  });

  it('put the original first and list it once', () => {
    const none = { value: 0, label: 'None', num: '' };
    expect(withOriginal([none], { value: 0, label: 'None', num: '' })).toEqual([{ value: 0, label: 'None', num: 'Original' }]);
    expect(withOriginal([none], null)).toEqual([none]);
  });

  it('pick a starter with Original first, and apply a number', () => {
    let picked = 0;
    const p = speciesPicker(view, 'Pokémon Emerald (U)', { label: 'Starter 1', current: 4, original: 1, apply: (v: number) => (picked = v) });
    expect(p.title).toBe('Choose starter 1');
    expect(p.specials).toEqual([{ value: 1, label: 'Bulbasaur', num: 'Original' }]);
    expect(p.current).toBe(4);
    p.apply(2);
    expect(picked).toBe(2);
  });
});
