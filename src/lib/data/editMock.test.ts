// The browser preview's game data and checks: every generation's rules, and the issues the editor shows.
import { describe, expect, it } from 'vitest';
import type { Edit } from '../generated/Edit';
import { originalEdit } from '../edits/changes';
import { mockEditableGame, mockValidate } from './editMock';
import { sampleRoms, type RomInfo } from './roms';

const [platinum, emerald, ultraMoon] = sampleRoms();
const asGen = (rom: RomInfo, genNum: number): RomInfo => ({ ...rom, genNum });
const check = (rom: RomInfo, edits: Edit[]) => {
  const view = mockEditableGame(rom);
  return mockValidate(view, { version: 1, game: view.game, edits });
};

describe('sample games', () => {
  it('have every kind of object, with their starters', () => {
    const view = mockEditableGame(emerald);
    expect(view.starters.map((s) => s.species)).toEqual([252, 255, 258]);
    for (const list of [view.trainers, view.wildSets, view.statics, view.trades, view.shops, view.itemPrices, view.fieldItems]) expect(list.length).toBeGreaterThan(0);
    expect(view.starterHeldItems).toHaveLength(1);
    expect(view.rules.wild.levels.every((l) => l === 'range')).toBe(true);
  });

  it('follow each generation’s rules', () => {
    expect(mockEditableGame(asGen(emerald, 1)).rules.prices.tms).toEqual({ max: 9_000, step: 1_000 });
    expect(mockEditableGame(asGen(emerald, 1)).rules.trades.originalTrainer).toBe(false);
    expect(mockEditableGame(asGen(emerald, 1)).starterHeldItems).toEqual([]);
    expect(mockEditableGame(asGen(emerald, 2)).rules.wild.levels[0]).toBe('single');
    expect(mockEditableGame(platinum).rules.shops.size.kind).toBe('shared');
    expect(mockEditableGame(asGen(emerald, 5)).rules.shops.size).toEqual({ kind: 'grow', max: 255 });
    expect(mockEditableGame(ultraMoon).rules.shops.size.kind).toBe('fixed');
    expect(mockEditableGame(ultraMoon).rules.wild.levels[0]).toBe('shared');
    expect(mockEditableGame(ultraMoon).itemPrices.every((p) => p.price % 10 === 0)).toBe(true);
  });

  it('come back the same every time', () => {
    expect(mockEditableGame(platinum)).toEqual(mockEditableGame(platinum));
  });
});

describe('preview checks', () => {
  it('find nothing wrong with edits that fit', () => {
    expect(check(emerald, [{ kind: 'shop', index: 0, items: [4, 13, 17] }, { kind: 'itemPrice', item: 13, price: 250 }])).toEqual([]);
  });

  it('keep shops of a fixed size at their size', () => {
    const view = mockEditableGame(ultraMoon);
    const shop = originalEdit(view, { kind: 'shop', index: 0 }) as Edit & { kind: 'shop' };
    const issues = check(ultraMoon, [{ ...shop, items: [...shop.items, 4] }]);
    expect(issues).toHaveLength(1);
    expect(issues[0]).toMatchObject({ target: { kind: 'shop', index: 0 }, field: 'items', severity: 'error' });
    expect(issues[0].message).toMatch(/keeps it at exactly 5\. Remove 1\.$/);
  });

  it('report the shared shop space as a whole', () => {
    const issues = check(platinum, [{ kind: 'shop', index: 1, items: Array(120).fill(4) }]);
    expect(issues).toEqual([{ target: null, field: 'shop space', severity: 'error', message: expect.stringMatching(/^The shop lists need \d+ bytes more/) }]);
  });

  it('check prices, teams, trades and wild levels', () => {
    const issues = check(emerald, [
      { kind: 'itemPrice', item: 12, price: 70_000 },
      { kind: 'trainer', index: 1, pokemon: [] },
      { kind: 'trade', index: 0, requested: 1, given: 1, nickname: 'ABCDEFGHIJK', otName: '', otId: 0, ivs: [], heldItem: null },
      { kind: 'wildSet', index: 0, slots: [{ species: 1, minLevel: 9, maxLevel: 3 }] }
    ]);
    expect(issues.map((i) => i.field)).toEqual(['price', 'team size', 'nickname', 'original trainer', 'levels']);
    expect(issues[0].message).toBe('Potion costs 70,000; this game stores at most 65,535.');
    expect(issues[1].message).toBe('Youngster Joey has no Pokémon; a team needs at least one.');
  });

  it('warn when an item the game needs where it is gets replaced', () => {
    const issues = check(emerald, [{ kind: 'fieldItem', index: 9, item: 4 }]);
    expect(issues).toEqual([{ target: { kind: 'fieldItem', index: 9 }, field: 'item', severity: 'warning', message: 'Field item 10 is TM05, which the game gives nowhere else; replacing it leaves the player without it.' }]);
  });

  it('say so for an object the game doesn’t have', () => {
    expect(check(emerald, [{ kind: 'static', index: 99, species: 1, level: 5 }])[0].message).toBe('Static encounter 100 isn’t in this game.');
  });
});
