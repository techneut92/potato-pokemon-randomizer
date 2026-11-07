// Picker dialogs for edit fields: the game's choices, with the original value first.
import type { EditableGame } from '../backend';
import type { Type } from '../generated/Type';
import { generationFilters, speciesEntries, withOriginal, type PickEntry, type PickFilters, type PickRequest } from '../picker';
import { speciesName } from './changes';
import { ITEM_CATS, itemCat, itemName, itemTag } from './items';

/** A field's picker, by what the field is called ("Starter 1") and its current and original values. */
export interface FieldPick<V> {
  label: string;
  current: V;
  /** `undefined` when the object is new (a Pokémon added to a team), so there is no original to offer. */
  original?: V;
  apply: (value: V) => void;
}

/** "Starter 1" → "Choose starter 1". */
export function chooseTitle(label: string): string {
  return 'Choose ' + label.charAt(0).toLowerCase() + label.slice(1);
}

/** The original value as the first special, unless the field has none. */
function original<V extends string | number | null>(f: FieldPick<V>, name: (v: V) => string, num = ''): PickEntry | null {
  return f.original === undefined ? null : { value: f.original, label: name(f.original), num };
}

/**
 * The species picker of an edit field. `none` names the choice of no species, for fields that may have none
 * (a trade that takes any Pokémon).
 */
export function speciesPicker<V extends number | null>(view: EditableGame, gameName: string, f: FieldPick<V>, none?: string): PickRequest {
  const name = (v: V) => (v === null ? (none ?? 'None') : speciesName(view, v));
  return {
    title: chooseTitle(f.label),
    lede: 'Every Pokémon in ' + gameName + ' can go here. Original is what the game has now.',
    hint: 'Search by name or number, e.g. eevee or 133',
    noun: 'Pokémon',
    entries: speciesEntries(view.species.map((s) => s.name)),
    specials: withOriginal(none ? [{ value: null, label: none, num: '' }] : [], original(f, name)),
    filters: generationFilters(view.generation),
    current: f.current,
    apply: (v) => f.apply((v === null ? null : Number(v)) as V)
  };
}

/** Pills by item category, for the categories the game has. */
export function itemFilters(names: readonly string[]): PickFilters {
  const cats = ITEM_CATS.filter(([id]) => id === 'all' || names.some((n) => itemCat(n) === id));
  return { label: 'Category', options: cats.map(([id, label]) => ({ label, test: (e: PickEntry) => id === 'all' || itemCat(e.label) === id })) };
}

/** The item picker of an edit field; with `none`, "None" (no item, `null`) is a choice too. */
export function itemPicker(view: EditableGame, gameName: string, f: FieldPick<number | null>, none = false): PickRequest {
  return {
    title: chooseTitle(f.label),
    lede: 'Every item ' + gameName + ' can sell, give or hide.',
    hint: 'Search items',
    noun: 'items',
    entries: view.items.map((i) => ({ value: i.id, label: i.name, num: itemTag(i.name) })),
    specials: withOriginal(none ? [{ value: null, label: 'None', num: '' }] : [], original(f, (v) => itemName(view, v))),
    filters: itemFilters(view.items.map((i) => i.name)),
    current: f.current,
    apply: (v) => f.apply(v === null ? null : Number(v))
  };
}

/** The type picker of an edit field; with `none`, "None" (`null`) makes a single type. */
export function typePicker(view: EditableGame, gameName: string, f: FieldPick<Type | null>, none = false): PickRequest {
  return {
    title: chooseTitle(f.label),
    lede: 'The types that exist in ' + gameName + '.',
    noun: 'types',
    entries: view.types.map((t) => ({ value: t, label: t, num: '' })),
    specials: withOriginal(none ? [{ value: null, label: 'None', num: '' }] : [], original(f, (v) => v ?? 'None')),
    current: f.current,
    apply: (v) => f.apply(v === null ? null : (String(v) as Type))
  };
}

/** A move's name by number; 0 is "Default" (the level-up moves). */
export function moveName(view: EditableGame, id: number): string {
  return id ? (view.moves[id] ?? 'Move ' + id) : 'Default';
}

/** The move picker of one of a trainer Pokémon's moves; "Default" (0) leaves the slot to its level-up moves. */
export function movePicker(view: EditableGame, f: FieldPick<number>): PickRequest {
  return {
    title: chooseTitle(f.label),
    lede: 'Default uses the move this Pokémon learns by level.',
    hint: 'Search moves by name or number',
    noun: 'moves',
    entries: view.moves.map((label, i) => ({ value: i, label, num: String(i) })).filter((e) => e.value > 0 && e.label),
    specials: withOriginal([{ value: 0, label: 'Default', num: '' }], original(f, (v) => moveName(view, v))),
    current: f.current,
    apply: (v) => f.apply(Number(v))
  };
}

/** An ability's name by number; 0 is no ability. */
export function abilityName(view: EditableGame, id: number): string {
  return id ? (view.abilities[id] ?? 'Ability ' + id) : 'None';
}

/** The ability picker of an ability slot; with `none`, "None" (0) leaves the slot empty. */
export function abilityPicker(view: EditableGame, gameName: string, f: FieldPick<number>, none = false): PickRequest {
  return {
    title: chooseTitle(f.label),
    lede: 'Every ability in ' + gameName + '.',
    hint: 'Search abilities',
    noun: 'abilities',
    entries: view.abilities.map((label, i) => ({ value: i, label, num: String(i) })).filter((e) => e.value > 0 && e.label),
    specials: withOriginal(none ? [{ value: 0, label: 'None', num: '' }] : [], original(f, (v) => abilityName(view, v))),
    current: f.current,
    apply: (v) => f.apply(Number(v))
  };
}
