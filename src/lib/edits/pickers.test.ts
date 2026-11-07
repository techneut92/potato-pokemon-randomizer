// The pickers of edit fields: species, items, types, moves and abilities.
import { describe, expect, it } from 'vitest';
import { fixtureGame } from './fixture';
import { abilityName, abilityPicker, itemFilters, itemPicker, moveName, movePicker, speciesPicker, typePicker } from './pickers';

const view = fixtureGame();
const game = 'Pokémon Emerald (U)';

describe('the species picker', () => {
  it('offers no original for a new object', () => {
    const p = speciesPicker(view, game, { label: 'Pokémon in slot 3', current: 2, apply: () => {} });
    expect(p.specials).toEqual([]);
    expect(p.entries).toHaveLength(4);
  });

  it('can offer no species, for a trade that takes any', () => {
    let picked: number | null = 1;
    const p = speciesPicker(view, game, { label: 'What trade 1 asks for', current: 1, original: null, apply: (v: number | null) => (picked = v) }, 'Any Pokémon');
    expect(p.specials).toEqual([{ value: null, label: 'Any Pokémon', num: 'Original' }]);
    p.apply(null);
    expect(picked).toBeNull();
    p.apply(3);
    expect(picked).toBe(3);
  });
});

describe('the item picker', () => {
  it('lists the game’s items with their category, original first', () => {
    const p = itemPicker(view, game, { label: 'Item 1', current: 13, original: 4, apply: () => {} });
    expect(p.title).toBe('Choose item 1');
    expect(p.entries.find((e) => e.value === 289)).toEqual({ value: 289, label: 'TM01', num: 'TM' });
    expect(p.entries.find((e) => e.value === 13)?.num).toBe('Med');
    expect(p.specials).toEqual([{ value: 4, label: 'Poké Ball', num: 'Original' }]);
    expect(p.noun).toBe('items');
  });

  it('offers None for a held item, as the original when there was none', () => {
    let picked: number | null = 0;
    const p = itemPicker(view, game, { label: 'Held item in slot 1', current: null, original: null, apply: (v) => (picked = v) }, true);
    expect(p.specials).toEqual([{ value: null, label: 'None', num: 'Original' }]);
    p.apply(null);
    expect(picked).toBeNull();
    p.apply(139);
    expect(picked).toBe(139);
  });

  it('filters by the categories the game has', () => {
    const f = itemFilters(view.items.map((i) => i.name));
    expect(f.options.map((o) => o.label)).toEqual(['All', 'Poké Balls', 'Medicine', 'Berries', 'TMs & HMs']);
    const tms = f.options[4];
    expect(tms.test({ value: 339, label: 'HM01', num: 'TM' })).toBe(true);
    expect(tms.test({ value: 13, label: 'Potion', num: 'Med' })).toBe(false);
  });
});

describe('the type picker', () => {
  it('lists the game’s types without a search', () => {
    const p = typePicker(view, game, { label: 'Type 1', current: 'Grass', original: 'Grass', apply: () => {} });
    expect(p.hint).toBeUndefined();
    expect(p.entries.map((e) => e.value)).toEqual(view.types);
    expect(p.specials).toEqual([{ value: 'Grass', label: 'Grass', num: 'Original' }]);
  });

  it('offers None for the second type', () => {
    let picked: string | null = 'x';
    const p = typePicker(view, game, { label: 'Type 2', current: 'Poison', original: 'Poison', apply: (t) => (picked = t) }, true);
    expect(p.specials.map((s) => s.label)).toEqual(['Poison', 'None']);
    p.apply(null);
    expect(picked).toBeNull();
    p.apply('Water');
    expect(picked).toBe('Water');
  });
});

describe('the move picker', () => {
  it('offers Default, the level-up moves, and every named move', () => {
    const p = movePicker(view, { label: 'Move 1 in slot 2', current: 0, original: 0, apply: () => {} });
    expect(p.specials).toEqual([{ value: 0, label: 'Default', num: 'Original' }]);
    expect(p.entries.map((e) => [e.value, e.label, e.num])).toEqual([
      [1, 'Tackle', '1'],
      [2, 'Growl', '2'],
      [3, 'Vine Whip', '3']
    ]);
    expect(moveName(view, 0)).toBe('Default');
    expect(moveName(view, 3)).toBe('Vine Whip');
  });

  it('keeps Default after another original', () => {
    const p = movePicker(view, { label: 'Move 1', current: 2, original: 1, apply: () => {} });
    expect(p.specials.map((s) => [s.label, s.num])).toEqual([
      ['Tackle', 'Original'],
      ['Default', '']
    ]);
  });
});

describe('the ability picker', () => {
  it('offers None only where a slot may be empty', () => {
    const first = abilityPicker(view, game, { label: 'Ability 1', current: 2, original: 2, apply: () => {} });
    expect(first.specials).toEqual([{ value: 2, label: 'Blaze', num: 'Original' }]);
    const second = abilityPicker(view, game, { label: 'Ability 2', current: 0, original: 0, apply: () => {} }, true);
    expect(second.specials).toEqual([{ value: 0, label: 'None', num: 'Original' }]);
    expect(second.entries.map((e) => e.label)).toEqual(['Overgrow', 'Blaze', 'Chlorophyll']);
    expect(abilityName(view, 0)).toBe('None');
  });
});
