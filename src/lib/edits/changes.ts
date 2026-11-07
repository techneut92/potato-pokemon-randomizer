// What the Changes list and the editor say about objects: names, roles, originals, and each edit's before and after.
import type { EditableGame } from '../backend';
import type { Edit } from '../generated/Edit';
import type { ObjectRef } from '../generated/ObjectRef';
import { itemName, money } from './items';
import { refKey, sectionOf, targetOf, type EditSection, type SectionId } from './sections';
import { trainerLabels } from './trainers';
import { splitTime } from './wild';

/** `#001`. */
export function dexNum(n: number): string {
  return '#' + String(n).padStart(3, '0');
}

/** A species' name by national number. */
export function speciesName(view: EditableGame, n: number): string {
  return view.species[n - 1]?.name ?? 'Pokémon ' + n;
}

/** The species a starter slot has in the game. */
export function originalStarter(view: EditableGame, slot: number): number {
  return view.starters.find((s) => s.slot === slot)?.species ?? 0;
}

/** What a starter slot is in the game, and a line about it. */
export function starterRole(slot: number, count: number, gen: number): { role: string; note: string } {
  // Yellow: the starter you get and the one the rival takes, which also decides his Eevee's line.
  if (gen === 1 && count === 2) {
    return slot === 0
      ? { role: 'Your starter', note: 'The Pokémon Professor Oak gives you.' }
      : { role: 'Rival’s starter', note: 'Also sets the Eevee in the rival’s first battle, which follows this choice.' };
  }
  // X and Y: three Kalos starters, then the Kanto starters from Professor Sycamore.
  if (gen === 6 && slot >= 3) return { role: 'Kanto starter ' + (slot - 2), note: 'One of the three Kanto starters you choose from later in the story.' };
  return { role: 'Starter ' + (slot + 1), note: 'One of the Pokémon you can pick at the start.' };
}

/** What a starter held item is called: one per starter, or one for all of them (Gen 3). */
export function heldItemLabel(view: EditableGame, slot: number): string {
  if (view.starterHeldItems.length === 1) return 'Starters’ held item';
  return starterRole(slot, view.starters.length, view.generation).role + ' held item';
}

/** A wild set's name with its time of day: "Route 29 (Morning)". */
export function wildSetName(view: EditableGame, index: number): string {
  const set = view.wildSets[index];
  if (!set) return 'Wild table ' + (index + 1);
  const { base, time } = splitTime(set.name);
  return time ? base + ' (' + time + ')' : base;
}

/**
 * The edit that leaves `ref` as the game has it, `undefined` when the game has no such object. An edit equal to it
 * changes nothing, so the store drops it.
 */
export function originalEdit(view: EditableGame, ref: ObjectRef): Edit | undefined {
  switch (ref.kind) {
    case 'starter': {
      const s = view.starters.find((x) => x.slot === ref.slot);
      return s && { kind: 'starter', ...s };
    }
    case 'starterHeldItem': {
      const s = view.starterHeldItems.find((x) => x.slot === ref.slot);
      return s && { kind: 'starterHeldItem', ...s };
    }
    case 'species': {
      const s = view.species[ref.number - 1];
      return s && { kind: 'species', ...s.value };
    }
    case 'trainer': {
      const t = view.trainers.find((x) => x.value.index === ref.index);
      return t && { kind: 'trainer', ...t.value };
    }
    case 'wildSet': {
      const w = view.wildSets[ref.index];
      return w && { kind: 'wildSet', ...w.value };
    }
    case 'static': {
      const s = view.statics[ref.index];
      return s && { kind: 'static', ...s.value };
    }
    case 'trade': {
      const t = view.trades[ref.index];
      return t && { kind: 'trade', ...t };
    }
    case 'shop': {
      const s = view.shops[ref.index];
      return s && { kind: 'shop', ...s.value };
    }
    case 'itemPrice': {
      const p = view.itemPrices.find((x) => x.item === ref.item);
      return p && { kind: 'itemPrice', ...p };
    }
    case 'fieldItem': {
      const f = view.fieldItems[ref.index];
      return f && { kind: 'fieldItem', ...f };
    }
  }
}

/** What an object is called in lists and messages. */
export function objectLabel(view: EditableGame, ref: ObjectRef, labels?: readonly string[]): string {
  switch (ref.kind) {
    case 'starter':
      return starterRole(ref.slot, view.starters.length, view.generation).role;
    case 'starterHeldItem':
      return heldItemLabel(view, ref.slot);
    case 'species':
      return speciesName(view, ref.number);
    case 'trainer': {
      const i = view.trainers.findIndex((t) => t.value.index === ref.index);
      return i < 0 ? 'Trainer ' + ref.index : (labels ?? trainerLabels(view.trainers))[i];
    }
    case 'wildSet':
      return wildSetName(view, ref.index);
    case 'static':
      return 'Static encounter ' + (ref.index + 1);
    case 'trade':
      return 'Trade ' + (ref.index + 1);
    case 'shop':
      return view.shops[ref.index]?.name ?? 'Shop ' + (ref.index + 1);
    case 'itemPrice':
      return itemName(view, ref.item);
    case 'fieldItem':
      return 'Field item ' + (ref.index + 1);
  }
}

/** "1 Pokémon changed", "3 slots changed". */
function changed(n: number, noun: string, plural = noun + 's'): string {
  return n + ' ' + (n === 1 ? noun : plural) + ' changed';
}

/** How many places differ between two lists of the same length. */
function diffs<T>(a: readonly T[], b: readonly T[]): number {
  return a.filter((x, i) => JSON.stringify(x) !== JSON.stringify(b[i])).length;
}

/** "A and B", "A, B and C". */
function listText(parts: string[]): string {
  return parts.length <= 1 ? (parts[0] ?? '') : parts.slice(0, -1).join(', ') + ' and ' + parts[parts.length - 1];
}

/** What an edit changes about its object, in one line: "Treecko → Mudkip", "3 → 4 Pokémon", "Price 1,200 → 1,000". */
export function changeDetail(view: EditableGame, edit: Edit, before: Edit | undefined): string {
  if (!before || before.kind !== edit.kind) return sectionOf(edit.kind).label;
  const sp = (n: number | null) => (n === null ? 'Any Pokémon' : speciesName(view, n));
  switch (edit.kind) {
    case 'starter':
      return sp((before as typeof edit).species) + ' → ' + sp(edit.species);
    case 'starterHeldItem':
      return itemName(view, (before as typeof edit).item) + ' → ' + itemName(view, edit.item);
    case 'species': {
      const b = before as typeof edit;
      const types = (t: typeof edit) => t.primaryType + (t.secondaryType ? ' / ' + t.secondaryType : '');
      const parts: string[] = [];
      if (types(b) !== types(edit)) parts.push('types');
      if (JSON.stringify(b.stats) !== JSON.stringify(edit.stats)) parts.push('base stats');
      if (JSON.stringify(b.abilities) !== JSON.stringify(edit.abilities)) parts.push('abilities');
      if (parts.length === 1 && parts[0] === 'types') return types(b) + ' → ' + types(edit);
      if (parts.length === 1 && parts[0] === 'base stats') {
        const total = (s: typeof edit.stats) => s.hp + s.attack + s.defense + s.speed + s.spAttack + s.spDefense;
        return 'Base stat total ' + total(b.stats) + ' → ' + total(edit.stats);
      }
      const text = listText(parts);
      return text.charAt(0).toUpperCase() + text.slice(1);
    }
    case 'trainer': {
      const b = (before as typeof edit).pokemon;
      if (b.length !== edit.pokemon.length) return b.length + ' → ' + edit.pokemon.length + ' Pokémon';
      return changed(diffs(b, edit.pokemon), 'Pokémon', 'Pokémon');
    }
    case 'wildSet':
      return changed(diffs((before as typeof edit).slots, edit.slots), 'slot');
    case 'static': {
      const b = before as typeof edit;
      if (b.species === edit.species) return 'Level ' + b.level + ' → ' + edit.level;
      if (b.level === edit.level) return sp(b.species) + ' → ' + sp(edit.species);
      return sp(b.species) + ' Lv. ' + b.level + ' → ' + sp(edit.species) + ' Lv. ' + edit.level;
    }
    case 'trade': {
      const b = before as typeof edit;
      if (b.given !== edit.given && JSON.stringify({ ...b, given: 0 }) === JSON.stringify({ ...edit, given: 0 })) return 'Gives ' + sp(b.given) + ' → ' + sp(edit.given);
      const parts: string[] = [];
      if (b.requested !== edit.requested) parts.push('what it asks for');
      if (b.given !== edit.given) parts.push('what it gives');
      if (b.nickname !== edit.nickname) parts.push('nickname');
      if (b.otName !== edit.otName || b.otId !== edit.otId) parts.push('original trainer');
      if (JSON.stringify(b.ivs) !== JSON.stringify(edit.ivs)) parts.push(view.rules.trades.maxIv === 15 ? 'DVs' : 'IVs');
      if (b.heldItem !== edit.heldItem) parts.push('held item');
      const text = listText(parts);
      return text.charAt(0).toUpperCase() + text.slice(1);
    }
    case 'shop': {
      const b = (before as typeof edit).items;
      if (b.length !== edit.items.length) return b.length + ' → ' + edit.items.length + ' items';
      return changed(diffs(b, edit.items), 'item');
    }
    case 'itemPrice':
      return 'Price ' + money((before as typeof edit).price) + ' → ' + money(edit.price);
    case 'fieldItem':
      return itemName(view, (before as typeof edit).item) + ' → ' + itemName(view, edit.item);
  }
}

/** One line of the Changes list. */
export interface Change {
  key: string;
  target: ObjectRef;
  section: EditSection;
  /** The object, e.g. "Starter 1". */
  label: string;
  /** What changed, e.g. "Treecko → Mudkip". */
  detail: string;
}

/** The Changes list: one line per edit, its before being the loaded game's value. */
export function describeChanges(edits: readonly Edit[], view: EditableGame): Change[] {
  const labels = trainerLabels(view.trainers);
  return edits.map((edit) => {
    const target = targetOf(edit);
    return { key: refKey(target), target, section: sectionOf(edit.kind), label: objectLabel(view, target, labels), detail: changeDetail(view, edit, originalEdit(view, target)) };
  });
}

/** How many objects a section lists: its size badge. */
export function sectionSize(view: EditableGame, id: SectionId): number {
  switch (id) {
    case 'starters':
      return view.starters.length;
    case 'mons':
      return view.species.length;
    case 'trainers':
      return view.trainers.length;
    case 'wild':
      return view.wildSets.length;
    case 'statics':
      return view.statics.length;
    case 'trades':
      return view.trades.length;
    case 'shops':
      return view.shops.length;
    case 'items':
      return view.itemPrices.length;
    case 'field':
      return view.fieldItems.length;
  }
}
