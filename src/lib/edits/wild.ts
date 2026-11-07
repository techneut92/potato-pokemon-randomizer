// Wild encounter sets in the editor: how each is met, the chance of each slot, and how its levels are stored.
import type { EditableWildSet, EncounterKind, WildLevels } from './types';

/** How a set is met, in words. */
export const KIND_LABEL: Record<EncounterKind, string> = {
  Walking: 'Grass or cave',
  Surfing: 'Surfing',
  Fishing: 'Fishing',
  Ambush: 'Ambush',
  Interact: 'Interact',
  Special: 'Special',
  Unused: 'Not used by the game'
};

/** The chance of each slot, where the game fixes them for the set's kind and size; `null` otherwise. */
export function slotChances(gen: number, kind: EncounterKind, n: number): string[] | null {
  if (gen === 1 && n === 10 && kind !== 'Fishing') return ['19.9%', '19.9%', '15.2%', '9.8%', '9.8%', '9.8%', '5.1%', '5.1%', '4.3%', '1.2%'];
  if (gen === 2 && n === 7 && kind === 'Walking') return ['30%', '30%', '20%', '10%', '5%', '4%', '1%'];
  if (gen >= 3 && gen <= 6 && n === 12 && kind === 'Walking') return ['20%', '20%', '10%', '10%', '10%', '10%', '5%', '5%', '4%', '4%', '1%', '1%'];
  return null;
}

/** A set's name and the time of day it is for, when its name ends in one: "Route 29 (Morning)". */
export function splitTime(name: string): { base: string; time: string } {
  const m = /^(.*) \((Morning|Day|Night)\)$/.exec(name);
  return m ? { base: m[1], time: m[2] } : { base: name, time: '' };
}

/** The chips under a set's name. */
export function wildChips(set: EditableWildSet): string[] {
  const chips = [KIND_LABEL[set.kind] ?? set.kind];
  const { time } = splitTime(set.name);
  if (time) chips.push(time);
  if (/Horde/.test(set.name)) chips.push('Horde: five at once');
  if (/Doubles/.test(set.name)) chips.push('Double battles');
  if (/Spots/.test(set.name)) chips.push('Shaking or rippling spots');
  if (set.postGame) chips.push('After the story');
  chips.push(set.value.slots.length + ' slots');
  return chips;
}

/** A group of slots in a Gen 7 set: the wild slots, the allies they call in, and the weather allies. */
export type SlotGroup = 'wild' | 'sos' | 'weather';

/** Which group slot `k` of a set of `n` slots is in. In Gen 7 every wild slot comes with 7 allies; weather allies follow. */
export function slotGroup(gen: number, n: number, k: number): SlotGroup {
  const nb = gen === 7 ? Math.floor(n / 8) : 0;
  if (nb === 0) return 'wild';
  if (k >= nb * 8) return 'weather';
  return k % 8 === 0 ? 'wild' : 'sos';
}

/** The groups a set has, with how many slots each, for the pills; empty when it has only wild slots. */
export function slotGroups(gen: number, n: number): { id: SlotGroup; label: string; count: number }[] {
  const nb = gen === 7 ? Math.floor(n / 8) : 0;
  if (nb === 0) return [];
  const groups: { id: SlotGroup; label: string; count: number }[] = [
    { id: 'wild', label: 'Wild slots', count: nb },
    { id: 'sos', label: 'SOS allies', count: nb * 7 },
    { id: 'weather', label: 'Weather allies', count: n - nb * 8 }
  ];
  return groups.filter((g) => g.count > 0);
}

/** What slot `k` is called: "3", or in Gen 7 "2" for a wild slot, "2.3" for an ally and "W1" for a weather ally. */
export function slotName(gen: number, n: number, k: number): string {
  const group = slotGroup(gen, n, k);
  const nb = Math.floor(n / 8);
  if (group === 'sos') return Math.floor(k / 8) + 1 + '.' + (k % 8);
  if (group === 'weather') return 'W' + (k - nb * 8 + 1);
  return String(gen === 7 && nb > 0 ? k / 8 + 1 : k + 1);
}

/** The line under an ally's species. */
export function slotRole(gen: number, n: number, k: number): string {
  const group = slotGroup(gen, n, k);
  if (group === 'sos') return 'Ally called by wild slot ' + (Math.floor(k / 8) + 1);
  if (group === 'weather') return 'Ally called in rain, hail or sandstorm';
  return '';
}

/** The note under a set: what the game fixes and how the levels work. */
export function wildNote(levels: WildLevels, gen: number): string {
  const base = 'The slot count' + (gen <= 2 ? ' and chances are' : ' is') + ' fixed by the game.';
  switch (levels) {
    case 'single':
      return base + ' Each slot has one level.' + (gen === 2 ? ' Grass and cave tables come in morning, day and night sets.' : '');
    case 'range':
      return base + ' Every slot has its own level range.';
    case 'shared':
      return gen === 7
        ? 'Each table has up to 10 wild slots. Every wild slot has 7 SOS allies it can call in, and some tables add allies for rain, hail or sandstorm. The level range is shared by the whole table.'
        : base + ' The level range is shared by the whole table.';
    case 'fixed':
      return base + ' The game sets the levels of this table; only the species can change.';
  }
}
