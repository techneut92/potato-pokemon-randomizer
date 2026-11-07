// The Changes list: what each kind of edit says it changes, the originals edits are compared with, and size badges.
import { describe, expect, it } from 'vitest';
import type { Edit } from '../generated/Edit';
import { changeDetail, describeChanges, heldItemLabel, objectLabel, originalEdit, sectionSize } from './changes';
import { fixtureGame } from './fixture';
import { EDIT_SECTIONS, sameValue, targetOf } from './sections';

const view = fixtureGame();
/** The detail of one edit made from its original by `change`. */
const detail = (ref: Parameters<typeof originalEdit>[1], change: (e: Edit) => void) => {
  const before = originalEdit(view, ref)!;
  const edit = JSON.parse(JSON.stringify(before)) as Edit;
  change(edit);
  return changeDetail(view, edit, before);
};

describe('originals', () => {
  it('are the game’s objects in the shape of their edit', () => {
    expect(originalEdit(view, { kind: 'starter', slot: 1 })).toEqual({ kind: 'starter', slot: 1, species: 4 });
    expect(originalEdit(view, { kind: 'trainer', index: 2 })).toEqual({ kind: 'trainer', index: 2, pokemon: [{ species: 2, level: 5, heldItem: null, moves: null }] });
    expect(originalEdit(view, { kind: 'itemPrice', item: 13 })).toEqual({ kind: 'itemPrice', item: 13, price: 300 });
    expect(originalEdit(view, { kind: 'shop', index: 1 })).toEqual({ kind: 'shop', index: 1, items: [289] });
    expect(originalEdit(view, { kind: 'trainer', index: 9 })).toBeUndefined();
    expect(originalEdit(view, { kind: 'itemPrice', item: 1 })).toBeUndefined();
  });

  it('name the object they edit', () => {
    for (const ref of [{ kind: 'species', number: 3 }, { kind: 'static', index: 1 }, { kind: 'fieldItem', index: 2 }, { kind: 'wildSet', index: 0 }] as const) {
      expect(targetOf(originalEdit(view, ref)!)).toEqual(ref);
    }
  });

  it('equal an unchanged object, so the store drops an edit back to it', () => {
    const before = originalEdit(view, { kind: 'trade', index: 0 })!;
    expect(sameValue(JSON.parse(JSON.stringify(before)), before)).toBe(true);
  });
});

describe('change descriptions', () => {
  it('give a starter’s and a held item’s before and after', () => {
    expect(detail({ kind: 'starter', slot: 0 }, (e) => e.kind === 'starter' && (e.species = 4))).toBe('Bulbasaur → Charmander');
    expect(detail({ kind: 'starterHeldItem', slot: 0 }, (e) => e.kind === 'starterHeldItem' && (e.item = 139))).toBe('None → Oran Berry');
  });

  it('say what changed about a species', () => {
    expect(detail({ kind: 'species', number: 4 }, (e) => e.kind === 'species' && (e.secondaryType = 'Flying'))).toBe('Fire → Fire / Flying');
    expect(detail({ kind: 'species', number: 1 }, (e) => e.kind === 'species' && (e.stats.hp = 55))).toBe('Base stat total 318 → 328');
    expect(detail({ kind: 'species', number: 1 }, (e) => e.kind === 'species' && (e.abilities = [3, 0]))).toBe('Abilities');
    expect(
      detail({ kind: 'species', number: 1 }, (e) => {
        if (e.kind !== 'species') return;
        e.primaryType = 'Water';
        e.stats.speed = 90;
        e.abilities = [3, 2];
      })
    ).toBe('Types, base stats and abilities');
  });

  it('count a team’s and a table’s changes', () => {
    expect(detail({ kind: 'trainer', index: 1 }, (e) => e.kind === 'trainer' && e.pokemon.pop())).toBe('2 → 1 Pokémon');
    expect(detail({ kind: 'trainer', index: 1 }, (e) => e.kind === 'trainer' && (e.pokemon[0].level = 20))).toBe('1 Pokémon changed');
    expect(detail({ kind: 'wildSet', index: 0 }, (e) => e.kind === 'wildSet' && (e.slots[0].species = 2) && (e.slots[1].maxLevel = 5))).toBe('2 slots changed');
  });

  it('give a static encounter’s species and level', () => {
    expect(detail({ kind: 'static', index: 0 }, (e) => e.kind === 'static' && (e.species = 1))).toBe('Venusaur → Bulbasaur');
    expect(detail({ kind: 'static', index: 0 }, (e) => e.kind === 'static' && (e.level = 50))).toBe('Level 70 → 50');
    expect(detail({ kind: 'static', index: 0 }, (e) => e.kind === 'static' && (e.level = 50) && (e.species = 1))).toBe('Venusaur Lv. 70 → Bulbasaur Lv. 50');
  });

  it('list what changed about a trade', () => {
    expect(detail({ kind: 'trade', index: 0 }, (e) => e.kind === 'trade' && (e.given = 2))).toBe('Gives Charmander → Ivysaur');
    expect(detail({ kind: 'trade', index: 0 }, (e) => e.kind === 'trade' && (e.nickname = 'POTATO') && (e.ivs = [31, 31, 31, 31, 31, 31]))).toBe('Nickname and IVs');
    const asks = detail({ kind: 'trade', index: 0 }, (e) => {
      if (e.kind !== 'trade') return;
      e.requested = null;
      e.otId = 1;
      e.heldItem = 13;
    });
    expect(asks).toBe('What it asks for, original trainer and held item');
  });

  it('give a shop’s size, a price and a field item', () => {
    expect(detail({ kind: 'shop', index: 0 }, (e) => e.kind === 'shop' && e.items.push(17))).toBe('2 → 3 items');
    expect(detail({ kind: 'shop', index: 0 }, (e) => e.kind === 'shop' && e.items.reverse())).toBe('2 items changed');
    expect(detail({ kind: 'itemPrice', item: 17 }, (e) => e.kind === 'itemPrice' && (e.price = 1200))).toBe('Price 3,000 → 1,200');
    expect(detail({ kind: 'fieldItem', index: 0 }, (e) => e.kind === 'fieldItem' && (e.item = 4))).toBe('Potion → Poké Ball');
  });

  it('fall back to the section for an object the game doesn’t have', () => {
    expect(changeDetail(view, { kind: 'shop', index: 7, items: [4] }, undefined)).toBe('Shops');
  });
});

describe('the Changes list', () => {
  it('names each object as the editor does', () => {
    const changes = describeChanges(
      [
        { kind: 'trainer', index: 1, pokemon: [] },
        { kind: 'shop', index: 1, items: [289, 4] },
        { kind: 'itemPrice', item: 13, price: 100 },
        { kind: 'wildSet', index: 0, slots: [] },
        { kind: 'static', index: 1, species: 1, level: 5 },
        { kind: 'starterHeldItem', slot: 0, item: 139 }
      ],
      view
    );
    expect(changes.map((c) => [c.label, c.section.id])).toEqual([
      ['Leader Roxanne', 'trainers'],
      ['Lilycove Dept. Store 5F', 'shops'],
      ['Potion', 'items'],
      ['Route 101 Grass/Cave', 'wild'],
      ['Static encounter 2', 'statics'],
      ['Starters’ held item', 'starters']
    ]);
    expect(changes[2].detail).toBe('Price 300 → 100');
  });

  it('calls a held item per starter by the starter’s role', () => {
    const three = { ...view, starterHeldItems: [0, 1, 2].map((slot) => ({ slot, item: null })) };
    expect(heldItemLabel(three, 1)).toBe('Starter 2 held item');
    expect(objectLabel(view, { kind: 'trade', index: 0 })).toBe('Trade 1');
  });
});

describe('size badges', () => {
  it('count the objects of each section', () => {
    expect(Object.fromEntries(EDIT_SECTIONS.map((s) => [s.id, sectionSize(view, s.id)]))).toEqual({ starters: 3, mons: 4, trainers: 2, wild: 1, statics: 2, trades: 1, shops: 2, items: 4, field: 3 });
  });
});
