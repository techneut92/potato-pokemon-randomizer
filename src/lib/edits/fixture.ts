// A small game for the editor's tests: a few objects of every kind, with Gen 3 rules unless a test changes them.
import type { EditableGame, EditRules } from './types';

export function fixtureGame(rules: Partial<EditRules> = {}): EditableGame {
  return {
    game: { name: 'Emerald (U)', code: 'BPEE', version: 0 },
    generation: 3,
    types: ['Normal', 'Fire', 'Water', 'Grass', 'Poison', 'Flying'],
    species: [
      ['Bulbasaur', 'Grass', 'Poison'],
      ['Ivysaur', 'Grass', 'Poison'],
      ['Venusaur', 'Grass', 'Poison'],
      ['Charmander', 'Fire', null]
    ].map(([name, primaryType, secondaryType], i) => ({
      name: name as string,
      value: { number: i + 1, primaryType: primaryType as 'Grass', secondaryType: secondaryType as 'Poison' | null, stats: { hp: 45, attack: 49, defense: 49, speed: 45, spAttack: 65, spDefense: 65 }, abilities: [2, 0] }
    })),
    abilities: ['', 'Overgrow', 'Blaze', 'Chlorophyll'],
    moves: ['', 'Tackle', 'Growl', 'Vine Whip'],
    items: [
      { id: 1, name: 'Master Ball', tm: false },
      { id: 4, name: 'Poké Ball', tm: false },
      { id: 13, name: 'Potion', tm: false },
      { id: 17, name: 'Full Restore', tm: false },
      { id: 139, name: 'Oran Berry', tm: false },
      { id: 289, name: 'TM01', tm: true },
      { id: 339, name: 'HM01', tm: false }
    ],
    starters: [1, 4, 2].map((species, slot) => ({ slot, species })),
    starterHeldItems: [{ slot: 0, item: null }],
    trainerClasses: ['Leader', 'Youngster'],
    trainers: [
      { name: 'Roxanne', class: 0, className: 'Leader', tag: 'GYM1-LEADER', value: { index: 1, pokemon: [{ species: 1, level: 12, heldItem: null, moves: null }, { species: 4, level: 15, heldItem: 139, moves: null }] } },
      { name: 'Joey', class: 1, className: 'Youngster', tag: null, value: { index: 2, pokemon: [{ species: 2, level: 5, heldItem: null, moves: null }] } }
    ],
    wildSets: [{ name: 'Route 101 Grass/Cave', kind: 'Walking', rate: 20, postGame: false, value: { index: 0, slots: [{ species: 1, minLevel: 2, maxLevel: 3 }, { species: 4, minLevel: 2, maxLevel: 3 }] } }],
    statics: [
      { egg: false, value: { index: 0, species: 3, level: 70 } },
      { egg: true, value: { index: 1, species: 4, level: 5 } }
    ],
    trades: [{ index: 0, requested: 1, given: 4, nickname: 'MAKIT', otName: 'ELYSSA', otId: 49562, ivs: [5, 4, 5, 4, 4, 4], heldItem: null }],
    shops: [
      { name: 'Oldale Town Poké Mart', mainGame: true, special: false, value: { index: 0, items: [4, 13] } },
      { name: 'Lilycove Dept. Store 5F', mainGame: true, special: true, value: { index: 1, items: [289] } }
    ],
    itemPrices: [
      { item: 4, price: 200 },
      { item: 13, price: 300 },
      { item: 17, price: 3000 },
      { item: 289, price: 3000 }
    ],
    fieldItems: [
      { index: 0, item: 13 },
      { index: 1, item: 289 },
      { index: 2, item: 4 }
    ],
    rules: {
      shops: { size: { kind: 'grow', max: null } },
      prices: { items: { max: 65_535, step: 1 }, tms: { max: 65_535, step: 1 } },
      trainers: { heldItems: true, customMoves: true, trainers: [] },
      wild: { levels: ['range'], banned: [], bannedInSets: [] },
      trades: { texts: true, maxNicknameLength: 10, maxOtNameLength: 7, originalTrainer: true, maxOtId: 65_535, heldItem: true, ivs: 6, maxIv: 31, randomIv: null },
      fieldItems: { required: [289] },
      starterLinks: [],
      ...rules
    }
  };
}
