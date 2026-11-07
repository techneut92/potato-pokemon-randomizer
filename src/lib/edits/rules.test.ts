// How the sections follow a game's rules: shop growth, price formats, teams, wild levels and trades.
import { describe, expect, it } from 'vitest';
import { clampNum, itemCat, money, priceLimit, priceProblem, priceRuleText, readPrice } from './items';
import { fixtureGame } from './fixture';
import { moveItem, sharedUse, shopCountText, shopProblem, shopRuleText, shopSpace, spaceProblem, spaceText } from './shops';
import { ivLabels, ivLegend, readIv, shownIv, tradeNote } from './trades';
import { addedPokemon, inFilter, limitsOf, tagLabel, teamNote, trainerFilters, trainerLabels, withMove } from './trainers';
import type { EditableTrainer, ShopSize, TradeRules } from './types';
import { slotChances, slotGroups, slotName, slotRole, splitTime, wildChips, wildNote } from './wild';

describe('shops', () => {
  const grow: ShopSize = { kind: 'grow', max: null };
  const capped: ShopSize = { kind: 'grow', max: 255 };
  const fixed: ShopSize = { kind: 'fixed' };
  const shrink: ShopSize = { kind: 'shrink' };
  const shared: ShopSize = { kind: 'shared', bytes: 40, costs: [{ base: 0, perItem: 4 }, { base: 2, perItem: 2 }], onlyChanged: false, max: 255 };

  it('grow freely where the game moves them to free space', () => {
    expect(shopProblem('Mart', 300, 5, grow)).toBe('');
    expect(shopRuleText(grow, 3)).toBe('A shop that grows moves to free space in the ROM, so shops can grow freely.');
  });

  it('hold at most the game’s limit', () => {
    expect(shopProblem('Mart', 255, 5, capped)).toBe('');
    expect(shopProblem('Mart', 256, 5, capped)).toBe('Mart sells 256 items; a shop holds at most 255.');
    expect(shopRuleText(capped, 1)).toBe('A shop that grows moves to free space in the ROM, so shops can grow. Each shop holds at most 255 items.');
    expect(shopRuleText(capped, 5)).toBe('Every shop is its own list, so shops can grow. Each shop holds at most 255 items.');
  });

  it('keep their size where the game fixes it, saying what to do', () => {
    expect(shopProblem('Mart', 6, 6, fixed)).toBe('');
    expect(shopProblem('Mart', 8, 6, fixed)).toBe('Mart sells 8 items, but this game keeps it at exactly 6. Remove 2.');
    expect(shopProblem('Mart', 5, 6, fixed)).toBe('Mart sells 5 items, but this game keeps it at exactly 6. Add 1.');
    expect(shopCountText(5, 6, fixed)).toBe('5 items of 6 (fixed)');
    expect(shopRuleText(fixed, 7)).toMatch(/^This game keeps every shop at its original size/);
  });

  it('only shrink where the game has no room to move them to', () => {
    expect(shopProblem('Mart', 4, 6, shrink)).toBe('');
    expect(shopProblem('Mart', 7, 6, shrink)).toBe('Mart sells 7 items, but this game can’t give it more than its 6. Remove 1.');
    expect(shopCountText(1, 6, shrink)).toBe('1 item of at most 6');
  });

  it('always need an item', () => {
    for (const size of [grow, capped, fixed, shrink, shared]) expect(shopProblem('Mart', 0, 3, size)).toBe('Mart sells nothing. A shop needs at least one item.');
  });

  it('share an area by bytes, the progressive shop taking more per item', () => {
    const items = [[1, 2, 3], [4, 5]];
    const space = shopSpace(shared, (i) => items[i], () => false)!;
    expect(space).toEqual({ cap: 40, used: 12 + 6, free: 22 });
    expect(spaceText(space)).toBe('22 bytes free');
    expect(spaceProblem(space)).toBe('');
    const over = shopSpace(shared, (i) => (i === 0 ? Array(8).fill(1) : [1, 2, 3, 4]), () => false)!;
    expect(spaceText(over)).toBe('over by 2 bytes');
    expect(spaceProblem(over)).toBe('The shop lists need 2 bytes more than the ROM has room for. Remove items from any shop.');
    expect(shopSpace(grow, () => [], () => false)).toBeNull();
    expect(shopRuleText(shared, 4, 22)).toBe(
      'The shop lists share a small reserved area with 22 bytes to spare. An item takes 4 bytes in the progressive shop, where it keeps its badge requirement and new items are sold from the start, and 2 bytes in every other counter.'
    );
  });

  it('count only changed shops when the game leaves the others where they are', () => {
    const onlyChanged: ShopSize = { ...shared, onlyChanged: true } as ShopSize;
    if (onlyChanged.kind !== 'shared') throw new Error();
    expect(sharedUse(onlyChanged, () => [1, 2], (i) => i === 1)).toBe(6);
  });

  it('move items up and down', () => {
    expect(moveItem([1, 2, 3], 2, 1)).toEqual([1, 3, 2]);
    expect(moveItem([1, 2, 3], 0, -1)).toEqual([1, 2, 3]);
  });
});

describe('prices', () => {
  it('follow the game’s limit and step', () => {
    const gen3 = { max: 65_535, step: 1 };
    const gen5 = { max: 655_350, step: 10 };
    expect(priceProblem('Potion', 65_535, gen3)).toBe('');
    expect(priceProblem('Potion', 65_536, gen3)).toBe('Potion costs 65,536; this game stores at most 65,535.');
    expect(priceProblem('Potion', 305, gen5)).toBe('Potion costs 305; prices in this game go in steps of 10.');
    expect(priceProblem('Potion', 310, gen5)).toBe('');
    expect(priceRuleText(gen3, false)).toBe('Prices in this game go from 0 to 65,535.');
    expect(priceRuleText(gen5, false)).toBe('Prices in this game are stored in steps of 10, from 0 to 655,350.');
  });

  it('store Gen 1 TMs in whole thousands', () => {
    const view = fixtureGame({ prices: { items: { max: 999_999, step: 1 }, tms: { max: 9_000, step: 1_000 } } });
    expect(priceLimit(view, 289)).toEqual({ max: 9_000, step: 1_000 });
    expect(priceLimit(view, 13)).toEqual({ max: 999_999, step: 1 });
    expect(priceProblem('TM01', 2500, priceLimit(view, 289), true)).toBe('TM01 costs 2,500; TM prices in this game are whole thousands.');
    expect(priceProblem('TM01', 10_000, priceLimit(view, 289), true)).toBe('TM01 costs 10,000; this game stores at most 9,000.');
    expect(priceRuleText(priceLimit(view, 289), true)).toBe('TM prices in this game are stored in whole thousands, from 0 to 9,000.');
  });

  it('read what is typed, and show thousands', () => {
    expect(readPrice('1,200')).toBe(1200);
    expect(readPrice('abc')).toBe(0);
    expect(readPrice('99999999')).toBe(9_999_999);
    expect(money(1200)).toBe('1,200');
    expect(money(655350)).toBe('655,350');
    expect(clampNum('300', 1, 255)).toBe(255);
    expect(clampNum('', 1, 100)).toBe(1);
  });

  it('put items in categories', () => {
    expect(['Ultra Ball', 'Full Restore', 'Oran Berry', 'HP Up', 'X Attack', 'Fire Stone', 'TM05', 'HM01', 'Nugget'].map(itemCat)).toEqual(['ball', 'med', 'berry', 'boost', 'battle', 'stone', 'tm', 'tm', 'other']);
  });
});

describe('trainers', () => {
  const t = (name: string, className: string, tag: string | null, index: number): EditableTrainer => ({ name, class: 0, className, tag, value: { index, pokemon: [] } });
  const trainers = [t('Roxanne', 'Leader', 'GYM1-LEADER', 1), t('', 'Rival1', 'RIVAL1-0', 2), t('', 'Rival2', 'RIVAL1-1', 3), t('', 'Grunt', null, 4), t('Joey', 'Youngster', null, 5)];

  it('are called by class and name, or numbered within a nameless class', () => {
    expect(trainerLabels(trainers)).toEqual(['Leader Roxanne', 'Rival #1', 'Rival #2', 'Grunt', 'Youngster Joey']);
  });

  it('get their story role from their tag', () => {
    expect(['GYM3-LEADER', 'GYM3', 'RIVAL2-1', 'FRIEND1-0', 'ELITE4', 'CHAMPION', 'UBER', 'THEMED:SPROUTTOWER', 'THEMED:AQUA-LEADER2', null].map(tagLabel)).toEqual([
      'Gym 3 leader',
      'Gym 3 trainer',
      'Rival battle 2',
      'Friend battle 1',
      'Elite Four 4',
      'Champion',
      'Postgame',
      'Story: Sprout Tower',
      'Story: Aqua',
      ''
    ]);
  });

  it('filter by role, offering only filters with trainers', () => {
    expect(trainerFilters(trainers).map(([id]) => id)).toEqual(['all', 'gym', 'rival']);
    expect(inFilter('elite', 'CHAMPION')).toBe(true);
    expect(inFilter('story', 'GIO1')).toBe(true);
    expect(inFilter('gym', null)).toBe(false);
  });

  it('take their limits from the rules, or the edit model’s', () => {
    const view = fixtureGame({ trainers: { heldItems: true, customMoves: false, trainers: [{ index: 1, minPokemon: 2, maxPokemon: 3, sharesBattle: true, levels: false, heldItems: false, customMoves: false, starterSlot: null }] } });
    expect(limitsOf(view, 1)).toMatchObject({ minPokemon: 2, maxPokemon: 3, levels: false });
    expect(limitsOf(view, 2)).toMatchObject({ minPokemon: 1, maxPokemon: 6, heldItems: true, customMoves: false });
    expect(teamNote(limitsOf(view, 1), 3)).toBe('This trainer’s Pokémon can’t hold items or have moves of their own. The game sets the levels of this trainer’s Pokémon.');
    expect(teamNote(limitsOf(view, 2), 1)).toMatch(/^Trainers in this game carry no held items/);
  });

  it('add a copy of the last Pokémon without its item or moves', () => {
    expect(addedPokemon([{ species: 4, level: 15, heldItem: 139, moves: [1, 2, 0, 0] }])).toEqual({ species: 4, level: 15, heldItem: null, moves: null });
    expect(addedPokemon([])).toEqual({ species: 25, level: 5, heldItem: null, moves: null });
  });

  it('keep moves of their own until all four are Default again', () => {
    expect(withMove(null, 1, 33)).toEqual([0, 33, 0, 0]);
    expect(withMove([0, 33, 0, 0], 1, 0)).toBeNull();
    expect(withMove([1, 33, 0, 0], 1, 0)).toEqual([1, 0, 0, 0]);
  });
});

describe('wild Pokémon', () => {
  it('show the chances the game fixes', () => {
    expect(slotChances(3, 'Walking', 12)?.[0]).toBe('20%');
    expect(slotChances(2, 'Walking', 7)).toHaveLength(7);
    expect(slotChances(1, 'Fishing', 10)).toBeNull();
    expect(slotChances(7, 'Walking', 12)).toBeNull();
  });

  it('split the time of day off a set’s name into a chip', () => {
    expect(splitTime('Route 29 (Morning)')).toEqual({ base: 'Route 29', time: 'Morning' });
    const set = { name: 'Route 29 Grass/Cave (Night)', kind: 'Walking' as const, rate: 10, postGame: true, value: { index: 0, slots: [{ species: 1, minLevel: 2, maxLevel: 2 }] } };
    expect(wildChips(set)).toEqual(['Grass or cave', 'Night', 'After the story', '1 slots']);
  });

  it('group the allies of Gen 7 tables', () => {
    expect(slotGroups(7, 19)).toEqual([
      { id: 'wild', label: 'Wild slots', count: 2 },
      { id: 'sos', label: 'SOS allies', count: 14 },
      { id: 'weather', label: 'Weather allies', count: 3 }
    ]);
    expect(slotGroups(3, 12)).toEqual([]);
    expect([0, 8, 9, 16].map((k) => slotName(7, 19, k))).toEqual(['1', '2', '2.1', 'W1']);
    expect(slotRole(7, 19, 9)).toBe('Ally called by wild slot 2');
    expect(slotName(3, 12, 4)).toBe('5');
  });

  it('describe how the table keeps its levels', () => {
    expect(wildNote('single', 1)).toBe('The slot count and chances are fixed by the game. Each slot has one level.');
    expect(wildNote('range', 4)).toBe('The slot count is fixed by the game. Every slot has its own level range.');
    expect(wildNote('shared', 7)).toMatch(/The level range is shared by the whole table\.$/);
    expect(wildNote('fixed', 5)).toMatch(/only the species can change\.$/);
  });
});

describe('trades', () => {
  const gen2: TradeRules = { texts: true, maxNicknameLength: 10, maxOtNameLength: 7, originalTrainer: true, maxOtId: 65_535, heldItem: true, ivs: 4, maxIv: 15, randomIv: null };
  const gen5: TradeRules = { ...gen2, ivs: 6, maxIv: 31, randomIv: 255 };

  it('label DVs and IVs', () => {
    expect(ivLabels(4)).toEqual(['Atk', 'Def', 'Spe', 'Spc']);
    expect(ivLabels(6)).toEqual(['HP', 'Atk', 'Def', 'Spe', 'SpA', 'SpD']);
    expect(ivLegend(gen2)).toBe('DVs (0–15)');
    expect(ivLegend(gen5)).toBe('IVs (0–31, empty means random)');
  });

  it('read an empty IV as random where the game has one', () => {
    expect(readIv('', gen5)).toBe(255);
    expect(shownIv(255, gen5)).toBe('');
    expect(readIv('40', gen5)).toBe(31);
    expect(readIv('', gen2)).toBe(0);
    expect(readIv('20', gen2)).toBe(15);
  });

  it('say which fields the game’s trades have', () => {
    expect(tradeNote(gen2)).toBe('Nicknames take up to 10 characters and original trainer names up to 7.');
    expect(tradeNote({ ...gen2, originalTrainer: false, heldItem: false, ivs: 0, maxOtNameLength: 0 })).toBe('Nicknames take up to 10 characters. Trades in this game set no original trainer, IVs or held item.');
    expect(tradeNote({ ...gen2, texts: false })).toBe('Nicknames and original trainer names stay as they are in this game.');
    expect(tradeNote({ ...gen2, texts: false, originalTrainer: false, heldItem: false, ivs: 0, maxOtNameLength: 0 })).toBe('Nicknames stay as they are in this game. Trades in this game set no original trainer, IVs or held item.');
  });
});
