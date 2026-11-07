// The manual editor's sections, and how an edit names the object it changes (ADR 0008).
import type { Edit } from '../generated/Edit';
import type { ObjectRef } from '../generated/ObjectRef';

export type EditKind = Edit['kind'];
export type SectionId = 'starters' | 'mons' | 'trainers' | 'wild' | 'statics' | 'trades' | 'shops' | 'items' | 'field';

export interface EditSection {
  id: SectionId;
  label: string;
  icon: string;
  desc: string;
  /** The edit kinds this section makes. */
  kinds: readonly EditKind[];
  /** What one object of the section is called, for links such as "Open starter". */
  noun: string;
  /** The placeholder of the search over the section's list; sections without one show all objects at once. */
  hint?: string;
  /** What the list's count counts: "386 species". */
  plural?: string;
}

export const EDIT_SECTIONS: readonly EditSection[] = [
  { id: 'starters', label: 'Starters', icon: 'M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1-4.4-4.3 6.1-.9z', desc: 'The Pokémon you can start with.', kinds: ['starter', 'starterHeldItem'], noun: 'starter' },
  { id: 'mons', label: 'Pokémon', icon: 'M4 19V9M10 19V5M16 19v-7M22 19H2', desc: 'Types, abilities and base stats of every species.', kinds: ['species'], noun: 'Pokémon', hint: 'Search Pokémon or type', plural: 'species' },
  { id: 'trainers', label: 'Trainers', icon: 'M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0', desc: 'Every trainer’s team.', kinds: ['trainer'], noun: 'trainer', hint: 'Search trainers or Pokémon', plural: 'trainers' },
  { id: 'wild', label: 'Wild Pokémon', icon: 'M5 21c0-6 3-10 7-12M12 9c4-1 7 0 8 3-3 2-6 2-8-3zM12 9C11 5 8 3 4 4c0 4 3 6 8 5z', desc: 'Every wild encounter table.', kinds: ['wildSet'], noun: 'table', hint: 'Search areas or Pokémon', plural: 'tables' },
  { id: 'statics', label: 'Static encounters', icon: 'M12 2v4M12 18v4M2 12h4M18 12h4M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8z', desc: 'Gifts and one-off battles, such as legendaries.', kinds: ['static'], noun: 'encounter' },
  { id: 'trades', label: 'In-game trades', icon: 'M7 7h13l-3-3M17 17H4l3 3', desc: 'What each trade asks for and what it gives.', kinds: ['trade'], noun: 'trade' },
  { id: 'shops', label: 'Shops', icon: 'M3 9l1.5-5h15L21 9M3 9h18v11H3zM9 20v-6h6v6', desc: 'What every shop counter sells.', kinds: ['shop'], noun: 'shop', hint: 'Search shops or items', plural: 'shops' },
  { id: 'items', label: 'Items', icon: 'M3 12V4h8l10 10-8 8zM7.5 7.5h.01', desc: 'What every item costs. A price belongs to the item for the whole game; selling gives back half.', kinds: ['itemPrice'], noun: 'item', hint: 'Search items', plural: 'items' },
  { id: 'field', label: 'Field items', icon: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM3 12h6M15 12h6M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z', desc: 'Item balls lying around the world, in the order the game stores them.', kinds: ['fieldItem'], noun: 'field item' }
];

/** The object an edit changes; the same as `Edit::target()` in `ppr-model`. */
export function targetOf(edit: Edit): ObjectRef {
  switch (edit.kind) {
    case 'starter':
      return { kind: 'starter', slot: edit.slot };
    case 'starterHeldItem':
      return { kind: 'starterHeldItem', slot: edit.slot };
    case 'species':
      return { kind: 'species', number: edit.number };
    case 'itemPrice':
      return { kind: 'itemPrice', item: edit.item };
    case 'trainer':
    case 'wildSet':
    case 'static':
    case 'trade':
    case 'shop':
    case 'fieldItem':
      return { kind: edit.kind, index: edit.index };
  }
}

/** The number that tells objects of one kind apart. */
export function refId(ref: ObjectRef): number {
  if ('slot' in ref) return ref.slot;
  if ('number' in ref) return ref.number;
  if ('item' in ref) return ref.item;
  return ref.index;
}

/** A key for maps and `{#each}`: `starter:0`, `trainer:7`, … */
export function refKey(ref: ObjectRef): string {
  return ref.kind + ':' + refId(ref);
}

export function sameRef(a: ObjectRef, b: ObjectRef): boolean {
  return a.kind === b.kind && refId(a) === refId(b);
}

/** The section that edits objects of `kind`. */
export function sectionOf(kind: EditKind): EditSection {
  return EDIT_SECTIONS.find((s) => s.kinds.includes(kind)) ?? EDIT_SECTIONS[0];
}

/** "1 manual edit", "3 manual edits". */
export function editsText(n: number): string {
  return n + ' manual ' + (n === 1 ? 'edit' : 'edits');
}

/** The progress step of a run that applies `n` edits. */
export function applyingStep(n: number): string {
  return 'Applying ' + editsText(n);
}

/** The steps a run shows: `steps`, with applying the edits before the last one (writing the ROM). */
export function runSteps(steps: readonly string[], edits: number): string[] {
  if (!edits || !steps.length) return [...steps];
  return [...steps.slice(0, -1), applyingStep(edits), steps[steps.length - 1]];
}

/** Whether two edit values are the same, field by field. */
export function sameValue(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  return ka.length === kb.length && ka.every((k) => sameValue((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]));
}

/** A plain copy of an edit value, to change without touching the one the store keeps. */
export function copy<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}
