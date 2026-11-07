// The sample games' data for the manual editor in the browser preview, and a stand-in for the backend's checks.
// The data is made up but shaped like a real game of the generation: its rules follow what each generation's
// handler reports, so every field and limit of the editor can be tried without a ROM.
import type { Edit } from '../generated/Edit';
import type { EditList } from '../generated/EditList';
import type { Type } from '../generated/Type';
import type { RomInfo } from './roms';
import { DEX } from './dex';
import { objectLabel, originalEdit } from '../edits/changes';
import { isTm, itemName, priceLimit, priceProblem } from '../edits/items';
import { refKey, targetOf } from '../edits/sections';
import { shopProblem, shopSpace, spaceProblem } from '../edits/shops';
import { limitsOf, trainerLabels } from '../edits/trainers';
import type { EditableGame, EditableTrainer, EditableWildSet, EditIssue, EditRules, EncounterKind, ShopSize } from '../edits/types';

const TYPES: Type[] = ['Normal', 'Fighting', 'Flying', 'Poison', 'Ground', 'Rock', 'Bug', 'Ghost', 'Steel', 'Fire', 'Water', 'Grass', 'Electric', 'Psychic', 'Ice', 'Dragon', 'Dark', 'Fairy'];

/** Items with their price in the sample games; 0 is no price. */
const ITEMS: [string, number][] = [
  ['Master Ball', 0], ['Ultra Ball', 1200], ['Great Ball', 600], ['Poké Ball', 200], ['Net Ball', 1000], ['Dive Ball', 1000], ['Nest Ball', 1000], ['Repeat Ball', 1000], ['Timer Ball', 1000], ['Luxury Ball', 1000], ['Premier Ball', 200],
  ['Potion', 300], ['Antidote', 100], ['Burn Heal', 250], ['Ice Heal', 250], ['Awakening', 250], ['Parlyz Heal', 200], ['Full Restore', 3000], ['Max Potion', 2500], ['Hyper Potion', 1200], ['Super Potion', 700], ['Full Heal', 600], ['Revive', 1500], ['Max Revive', 4000],
  ['Fresh Water', 200], ['Soda Pop', 300], ['Lemonade', 350], ['Moomoo Milk', 500], ['Energy Powder', 500], ['Energy Root', 800], ['Heal Powder', 450], ['Revival Herb', 2800], ['Ether', 1200], ['Max Ether', 2000], ['Elixir', 3000], ['Max Elixir', 4500],
  ['HP Up', 9800], ['Protein', 9800], ['Iron', 9800], ['Carbos', 9800], ['Calcium', 9800], ['Rare Candy', 4800], ['PP Up', 9800], ['Zinc', 9800], ['PP Max', 9800],
  ['Guard Spec.', 700], ['Dire Hit', 650], ['X Attack', 500], ['X Defend', 550], ['X Speed', 350], ['X Accuracy', 950], ['X Special', 350], ['Poké Doll', 1000], ['Fluffy Tail', 1000],
  ['Super Repel', 500], ['Max Repel', 700], ['Escape Rope', 550], ['Repel', 350],
  ['Sun Stone', 2100], ['Moon Stone', 2100], ['Fire Stone', 2100], ['Thunder Stone', 2100], ['Water Stone', 2100], ['Leaf Stone', 2100],
  ['Cheri Berry', 20], ['Chesto Berry', 20], ['Pecha Berry', 20], ['Rawst Berry', 20], ['Oran Berry', 20], ['Sitrus Berry', 20],
  ['Leftovers', 200], ['Lucky Egg', 200], ['Quick Claw', 100], ['Silk Scarf', 100], ['Nugget', 10000],
  ['TM01', 3000], ['TM02', 3000], ['TM03', 3000], ['TM04', 3000], ['TM05', 1000], ['TM06', 3000], ['TM07', 3000], ['TM08', 3000], ['TM09', 3000], ['TM10', 3000],
  ['HM01', 0], ['HM02', 0], ['HM03', 0]
];
const ID = (name: string) => ITEMS.findIndex(([n]) => n === name) + 1;

const ABILITIES = ['', 'Stench', 'Drizzle', 'Speed Boost', 'Battle Armor', 'Sturdy', 'Damp', 'Limber', 'Sand Veil', 'Static', 'Volt Absorb', 'Water Absorb', 'Oblivious', 'Cloud Nine', 'Compound Eyes', 'Insomnia', 'Color Change', 'Immunity', 'Flash Fire', 'Shield Dust', 'Own Tempo', 'Suction Cups', 'Intimidate', 'Shadow Tag', 'Rough Skin', 'Wonder Guard', 'Levitate', 'Effect Spore', 'Synchronize', 'Clear Body', 'Natural Cure', 'Lightning Rod', 'Serene Grace', 'Swift Swim', 'Chlorophyll', 'Illuminate', 'Trace', 'Huge Power', 'Poison Point', 'Inner Focus', 'Magma Armor', 'Water Veil', 'Magnet Pull', 'Soundproof', 'Rain Dish', 'Sand Stream', 'Pressure', 'Thick Fat', 'Early Bird', 'Flame Body', 'Run Away', 'Keen Eye', 'Hyper Cutter', 'Pickup', 'Truant', 'Hustle', 'Cute Charm', 'Plus', 'Minus', 'Forecast', 'Sticky Hold', 'Shed Skin', 'Guts', 'Marvel Scale', 'Liquid Ooze', 'Overgrow', 'Blaze', 'Torrent', 'Swarm'];
const MOVES = ['', 'Pound', 'Karate Chop', 'Double Slap', 'Comet Punch', 'Mega Punch', 'Pay Day', 'Fire Punch', 'Ice Punch', 'Thunder Punch', 'Scratch', 'Vice Grip', 'Guillotine', 'Razor Wind', 'Swords Dance', 'Cut', 'Gust', 'Wing Attack', 'Whirlwind', 'Fly', 'Bind', 'Slam', 'Vine Whip', 'Stomp', 'Double Kick', 'Mega Kick', 'Jump Kick', 'Rolling Kick', 'Sand Attack', 'Headbutt', 'Horn Attack', 'Fury Attack', 'Horn Drill', 'Tackle', 'Body Slam', 'Wrap', 'Take Down', 'Thrash', 'Double-Edge', 'Tail Whip', 'Poison Sting', 'Twineedle', 'Pin Missile', 'Leer', 'Bite', 'Growl', 'Roar', 'Sing', 'Supersonic', 'Sonic Boom', 'Disable', 'Acid', 'Ember', 'Flamethrower', 'Mist', 'Water Gun', 'Hydro Pump', 'Surf', 'Ice Beam', 'Blizzard', 'Psybeam', 'Bubble Beam', 'Aurora Beam', 'Hyper Beam'];
const CLASSES = ['Youngster', 'Lass', 'Bug Catcher', 'Hiker', 'Swimmer', 'Leader', 'Elite Four', 'Champion', 'Rival', 'Team Grunt', 'Ace Trainer'];

/** A number from a seed, the same every time: the sample data must not change between loads. */
const pick = (seed: number, n: number) => Math.abs(Math.imul(seed + 1, 2654435761) >> 7) % n;

function rulesOf(gen: number, game: Omit<EditableGame, 'rules'>): EditRules {
  const later = gen >= 2;
  let size: ShopSize;
  if (gen <= 2 || gen === 5) size = { kind: 'grow', max: 255 };
  else if (gen === 3) size = { kind: 'grow', max: null };
  else if (gen === 4) {
    // The progressive shop keeps a badge requirement with each item; the area has 200 bytes to spare.
    const costs = game.shops.map((_, i) => (i === 0 ? { base: 0, perItem: 4 } : { base: 2, perItem: 2 }));
    const used = costs.reduce((n, c, i) => n + c.base + c.perItem * game.shops[i].value.items.length, 0);
    size = { kind: 'shared', bytes: used + 200, costs, onlyChanged: false, max: 255 };
  } else size = { kind: 'fixed' };
  const prices = gen === 1 ? { items: { max: 999_999, step: 1 }, tms: { max: 9_000, step: 1_000 } } : gen <= 4 ? { items: { max: 65_535, step: 1 }, tms: { max: 65_535, step: 1 } } : { items: { max: 655_350, step: 10 }, tms: { max: 655_350, step: 10 } };
  return {
    shops: { size },
    prices,
    trainers: {
      heldItems: later,
      customMoves: later,
      trainers: game.trainers.map((t) => ({ index: t.value.index, minPokemon: 1, maxPokemon: 6, sharesBattle: false, levels: true, heldItems: later, customMoves: later, starterSlot: t.tag?.startsWith('RIVAL') ? t.value.pokemon.length - 1 : null }))
    },
    wild: { levels: game.wildSets.map(() => (gen <= 2 ? 'single' : gen === 7 ? 'shared' : 'range')), banned: [], bannedInSets: [] },
    trades: { texts: true, maxNicknameLength: 10, maxOtNameLength: later ? 7 : 0, originalTrainer: later, maxOtId: gen === 1 ? 0 : gen === 7 ? 4_294_967_295 : 65_535, heldItem: later, ivs: gen === 1 ? 0 : gen === 2 ? 4 : 6, maxIv: gen <= 2 ? 15 : 31, randomIv: gen === 5 || gen === 7 ? 255 : gen === 6 ? 32 : null },
    fieldItems: { required: [ID('TM05')] },
    starterLinks: []
  };
}

function trainers(gen: number, dex: number): EditableTrainer[] {
  const out: EditableTrainer[] = [];
  const add = (cls: number, name: string, tag: string | null, size: number, level: number) => {
    const index = out.length + 1;
    const pokemon = Array.from({ length: size }, (_, k) => ({ species: 1 + pick(index * 7 + k, dex), level: level + k, heldItem: gen >= 2 && tag?.includes('LEADER') && k === size - 1 ? ID('Sitrus Berry') : null, moves: null }));
    out.push({ name, class: cls, className: CLASSES[cls], tag, value: { index, pokemon } });
  };
  ['Joey', 'Ben', 'Calvin'].forEach((n, i) => add(0, n, null, 1 + (i % 2), 4 + i));
  ['Robin', 'Tiana'].forEach((n, i) => add(1, n, null, 2, 6 + i));
  add(8, '', 'RIVAL1-0', 1, 5);
  add(8, '', 'RIVAL1-1', 1, 5);
  add(8, '', 'RIVAL1-2', 1, 5);
  for (let g = 1; g <= 8; g++) {
    add(gen === 1 ? 2 : 3, ['Rick', 'Doug', 'Ted', 'Nob', 'Jill', 'Parker', 'Elijah', 'Kim'][g - 1], 'GYM' + g, 2, 10 + g * 4);
    add(5, ['Rocky', 'Brawly', 'Watt', 'Flannery', 'Norm', 'Winona', 'Tate', 'Juan'][g - 1], 'GYM' + g + '-LEADER', Math.min(6, 2 + Math.floor(g / 2)), 12 + g * 5);
  }
  add(9, '', 'THEMED:AQUA', 2, 16);
  add(9, '', 'THEMED:AQUA-LEADER', 3, 30);
  ['Sidney', 'Phoebe', 'Glacia', 'Drake'].forEach((n, i) => add(6, n, 'ELITE' + (i + 1), 5, 50 + i * 2));
  add(7, 'Wallace', 'CHAMPION', 6, 58);
  add(10, 'Kiri', 'UBER', 6, 70);
  ['Ian', 'Owen'].forEach((n) => add(4, n, null, 3, 28));
  return out;
}

function wildSets(gen: number, dex: number): EditableWildSet[] {
  const sets: EditableWildSet[] = [];
  const add = (name: string, kind: EncounterKind, n: number, level: number, postGame = false) => {
    const slots = Array.from({ length: n }, (_, k) => {
      const lo = level + (k % 3);
      return { species: 1 + pick(sets.length * 31 + k, dex), minLevel: lo, maxLevel: gen <= 2 ? lo : lo + (gen === 7 ? 3 : 1) };
    });
    if (gen === 7) for (const s of slots) Object.assign(s, { minLevel: level, maxLevel: level + 3 });
    sets.push({ name, kind, rate: kind === 'Walking' ? 20 : 4, postGame, value: { index: sets.length, slots } });
  };
  const walk = gen === 1 ? 10 : gen === 2 ? 7 : gen === 7 ? 19 : 12;
  for (const [i, area] of ['Route 101', 'Route 102', 'Petalburg Woods', 'Route 104', 'Granite Cave'].entries()) {
    if (gen === 2 || gen === 4) for (const t of ['Morning', 'Day', 'Night']) add(area + ' Grass/Cave (' + t + ')', 'Walking', walk, 3 + i * 3);
    else add(area + ' Grass/Cave', 'Walking', walk, 3 + i * 3);
  }
  if (gen !== 7) {
    add('Route 104 Surfing', 'Surfing', 5, 20);
    add('Route 104 Fishing', 'Fishing', 10, 5);
  }
  add('Sky Pillar Grass/Cave', 'Walking', walk, 48, true);
  return sets;
}

/** A sample game's editable view. */
export function mockEditableGame(rom: RomInfo): EditableGame {
  const gen = rom.genNum;
  const dex = rom.dexSize;
  const names = DEX.slice(0, dex);
  const types = TYPES.filter((t) => (gen >= 6 || t !== 'Fairy') && (gen >= 2 || (t !== 'Steel' && t !== 'Dark')));
  const abilityCount = gen < 3 ? 0 : gen < 5 ? 2 : 3;
  const items = ITEMS.map(([name], i) => ({ id: i + 1, name, tm: name.startsWith('TM') }));
  const shops = [
    { name: 'Oldale Town Poké Mart', items: ['Poké Ball', 'Potion', 'Antidote', 'Parlyz Heal', 'Awakening'], special: false },
    { name: 'Petalburg City Poké Mart', items: ['Poké Ball', 'Potion', 'Antidote', 'Parlyz Heal', 'Awakening', 'Escape Rope', 'Repel', 'X Speed', 'X Attack', 'X Defend', 'Orange Mail'], special: false },
    { name: 'Lilycove Dept. Store 2F', items: ['Poké Ball', 'Great Ball', 'Ultra Ball', 'Escape Rope', 'Full Heal', 'Revive'], special: false },
    { name: 'Lilycove Dept. Store 3F', items: ['Protein', 'Iron', 'Calcium', 'Zinc', 'Carbos', 'HP Up'], special: true },
    { name: 'Lilycove Dept. Store 5F TMs', items: ['TM01', 'TM02', 'TM03', 'TM04', 'TM06', 'TM07'], special: true },
    { name: 'Herb Shop', items: ['Energy Powder', 'Energy Root', 'Heal Powder', 'Revival Herb'], special: true }
  ];
  const game: Omit<EditableGame, 'rules'> = {
    game: { name: rom.name, code: rom.code, version: null },
    generation: gen,
    types,
    species: names.map((name, i) => {
      const primaryType = types[pick(i, types.length)];
      const second = pick(i * 3 + 1, types.length * 2);
      const secondaryType = second < types.length && types[second] !== primaryType ? types[second] : null;
      const st = (k: number) => 30 + pick(i * 11 + k, 100);
      const abilities = abilityCount ? Array.from({ length: abilityCount }, (_, k) => (k === 1 && i % 3 ? 0 : 1 + pick(i * 5 + k, ABILITIES.length - 1))) : [];
      const special = st(4);
      return { name, value: { number: i + 1, primaryType, secondaryType, stats: { hp: st(0), attack: st(1), defense: st(2), speed: st(3), spAttack: special, spDefense: gen === 1 ? 0 : st(5) }, abilities } };
    }),
    abilities: abilityCount ? ABILITIES : [],
    moves: gen >= 2 ? MOVES : [''],
    items,
    starters: rom.starters.map((s, slot) => ({ slot, species: names.indexOf(s) + 1 })),
    starterHeldItems: gen === 1 ? [] : gen === 3 ? [{ slot: 0, item: null }] : rom.starters.map((_, slot) => ({ slot, item: gen === 2 ? ID('Oran Berry') : null })),
    trainerClasses: CLASSES,
    trainers: trainers(gen, dex),
    wildSets: wildSets(gen, dex),
    statics: [
      { egg: false, value: { index: 0, species: Math.min(dex, 382), level: 70 } },
      { egg: false, value: { index: 1, species: Math.min(dex, 383), level: 70 } },
      { egg: false, value: { index: 2, species: Math.min(dex, 384), level: 70 } },
      { egg: false, value: { index: 3, species: 185, level: 40 } },
      { egg: true, value: { index: 4, species: 360, level: 5 } },
      { egg: false, value: { index: 5, species: 351, level: 25 } }
    ].filter((s) => s.value.species <= dex),
    trades: [
      { requested: 280, given: 296, nickname: 'MAKIT', otName: 'ELYSSA', otId: 49562, heldItem: ID('X Attack') },
      { requested: 116, given: 222, nickname: 'CORSOLA', otName: 'KOBE', otId: 2194, heldItem: null },
      { requested: null, given: 52, nickname: 'MEOWTH', otName: 'SKYLAR', otId: 1239, heldItem: ID('Nugget') }
    ].map((t, index) => ({
      index,
      requested: t.requested === null ? null : Math.min(t.requested, dex),
      given: Math.min(t.given, dex),
      nickname: t.nickname,
      otName: gen >= 2 ? t.otName : '',
      otId: gen >= 2 ? t.otId : 0,
      ivs: gen === 1 ? [] : gen === 2 ? [10, 12, 8, 15] : gen >= 5 ? [31, 255, 20, 255, 255, 5] : [5, 4, 5, 4, 4, 4],
      heldItem: gen >= 2 ? t.heldItem : null
    })),
    shops: shops.map((s, index) => ({ name: s.name, mainGame: true, special: s.special, value: { index, items: s.items.map(ID).filter((id) => id > 0) } })),
    itemPrices: ITEMS.map(([name, price], i) => ({ item: i + 1, price, name }))
      .filter((p) => !(gen === 1 && p.name.startsWith('HM')))
      .map(({ item, price }) => ({ item, price: gen >= 5 ? Math.round(price / 10) * 10 : price })),
    fieldItems: Array.from({ length: 48 }, (_, index) => ({ index, item: index === 9 ? ID('TM05') : 1 + pick(index * 13, ITEMS.length - 3) }))
  };
  return { ...game, rules: rulesOf(gen, game) };
}

/**
 * A browser stand-in for the backend's validation: the checks of the edit model and of the game's rules that the
 * editor's fields don't already rule out, worded for the user. The starter messages are the backend's.
 */
export function mockValidate(view: EditableGame, list: EditList): EditIssue[] {
  const issues: EditIssue[] = [];
  const labels = trainerLabels(view.trainers);
  const edits = new Map(list.edits.map((e) => [refKey(targetOf(e)), e]));
  const current = <K extends Edit['kind']>(e: Edit & { kind: K }) => (edits.get(refKey(targetOf(e))) ?? e) as Edit & { kind: K };
  for (const edit of list.edits) {
    const target = targetOf(edit);
    const name = objectLabel(view, target, labels);
    const error = (field: string | null, message: string) => issues.push({ target, field, severity: 'error', message });
    const warning = (field: string | null, message: string) => issues.push({ target, field, severity: 'warning', message });
    const before = originalEdit(view, target);
    if (!before) {
      error(null, name + ' isn’t in this game.');
      continue;
    }
    switch (edit.kind) {
      case 'starter': {
        if (edit.species < 1 || edit.species > view.species.length) error('species', `Starter ${edit.slot + 1} isn’t a Pokémon in this game.`);
        else if (view.starters.some((s) => s.slot !== edit.slot && current({ kind: 'starter', ...s }).species === edit.species)) warning('species', `Starter ${edit.slot + 1} is the same Pokémon as another starter.`);
        break;
      }
      case 'species':
        if (edit.secondaryType === edit.primaryType) error('types', name + ': the second type is the same as the first; leave it empty for a single type.');
        if (edit.abilities.length && edit.abilities[0] === 0) error('abilities', name + ': the first ability slot can’t be empty.');
        break;
      case 'trainer': {
        const limits = limitsOf(view, edit.index);
        const n = edit.pokemon.length;
        if (n > limits.maxPokemon) error('team size', `${name} has ${n} Pokémon; this team holds at most ${limits.maxPokemon}.`);
        if (n < limits.minPokemon) error('team size', `${name} has ${n ? n + ' Pokémon' : 'no Pokémon'}; ${limits.minPokemon === 1 ? 'a team needs at least one' : 'this team needs at least ' + limits.minPokemon}.`);
        break;
      }
      case 'wildSet':
        if (edit.slots.some((s) => s.minLevel > s.maxLevel)) error('levels', name + ': a slot’s lowest level is above its highest.');
        break;
      case 'trade': {
        const r = view.rules.trades;
        if (!edit.nickname) error('nickname', name + ': the nickname is empty.');
        else if (edit.nickname.length > r.maxNicknameLength) error('nickname', `${name}: the nickname is longer than ${r.maxNicknameLength} characters.`);
        if (r.originalTrainer && !edit.otName) error('original trainer', name + ': the original trainer’s name is empty.');
        else if (edit.otName.length > r.maxOtNameLength) error('original trainer', `${name}: the original trainer’s name is longer than ${r.maxOtNameLength} characters.`);
        break;
      }
      case 'shop': {
        const p = shopProblem(name, edit.items.length, (before as typeof edit).items.length, view.rules.shops.size);
        if (p) error('items', p);
        break;
      }
      case 'itemPrice': {
        const p = priceProblem(itemName(view, edit.item), edit.price, priceLimit(view, edit.item), isTm(view, edit.item));
        if (p) error('price', p);
        break;
      }
      case 'fieldItem': {
        const was = (before as typeof edit).item;
        if (view.rules.fieldItems.required.includes(was) && edit.item !== was) warning('item', `${name} is ${itemName(view, was)}, which the game gives nowhere else; replacing it leaves the player without it.`);
        break;
      }
    }
  }
  const shopItems = (i: number) => current({ kind: 'shop', ...view.shops[i].value }).items;
  const space = shopSpace(view.rules.shops.size, shopItems, (i) => edits.has('shop:' + i));
  const p = spaceProblem(space);
  if (p) issues.push({ target: null, field: 'shop space', severity: 'error', message: p });
  return issues;
}
