// Items in the editor: names, categories, prices and the price formats a game can store.
import type { EditableGame, PriceFormat } from './types';

/** Item categories for filters, in the order the pills show them; the first shows everything. */
export const ITEM_CATS = [
  ['all', 'All'],
  ['ball', 'Poké Balls'],
  ['med', 'Medicine'],
  ['berry', 'Berries'],
  ['boost', 'Vitamins'],
  ['battle', 'Battle items'],
  ['stone', 'Stones'],
  ['tm', 'TMs & HMs'],
  ['other', 'Other']
] as const;
export type ItemCat = Exclude<(typeof ITEM_CATS)[number][0], 'all'>;

/** The tag in front of an item in pickers and buttons. */
const ITEM_SHORT: Record<ItemCat, string> = { ball: 'Ball', med: 'Med', berry: 'Berry', boost: 'Boost', battle: 'Battle', stone: 'Stone', tm: 'TM', other: '' };

/** An item's category, from its name. */
export function itemCat(name: string): ItemCat {
  if (/^(TM|HM)\d/.test(name)) return 'tm';
  if (/Ball$/.test(name)) return 'ball';
  if (/berry$/i.test(name)) return 'berry';
  if (/(Fire|Water|Thunder|Leaf|Moon|Sun|Shiny|Dusk|Dawn|Oval|Ice) ?Stone$/.test(name)) return 'stone';
  if (/^X |Guard Spec|Dire Hit|Poké Doll|Fluffy Tail/.test(name)) return 'battle';
  if (/HP Up|Protein|Iron|Carbos|Calcium|Zinc|Rare Candy|PP Up|PP Max/.test(name)) return 'boost';
  if (/Potion|Heal|Restore|Revive|Awakening|Antidote|Paralyz|Parlyz|Water$|Soda|Lemonade|Ether|Elixir|Elixer|Milk|Herb|Energy|Juice/.test(name)) return 'med';
  return 'other';
}

/** The label of an item category. */
export function catLabel(cat: ItemCat): string {
  return ITEM_CATS.find(([id]) => id === cat)?.[1] ?? 'Other';
}

export function itemTag(name: string): string {
  return ITEM_SHORT[itemCat(name)];
}

/** An item's name by number; `null` is no item. */
export function itemName(view: EditableGame, id: number | null): string {
  if (id === null) return 'None';
  return view.items.find((i) => i.id === id)?.name ?? 'Item ' + id;
}

/** `1,200`. */
export function money(n: number): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/** The price an item has in the game, or `null` when it has none (like HMs in Gen 1). */
export function originalPrice(view: EditableGame, id: number): number | null {
  return view.itemPrices.find((p) => p.item === id)?.price ?? null;
}

/** Whether an item is a TM, whose price some games store in another format. */
export function isTm(view: EditableGame, id: number): boolean {
  return view.items.find((i) => i.id === id)?.tm ?? false;
}

/** The prices an item can have in this game. */
export function priceLimit(view: EditableGame, id: number): PriceFormat {
  return isTm(view, id) ? view.rules.prices.tms : view.rules.prices.items;
}

/** What the game stores of a price, as a sentence. */
export function priceRuleText(limit: PriceFormat, tm: boolean): string {
  if (limit.step === 1000) return (tm ? 'TM prices' : 'Prices') + ' in this game are stored in whole thousands, from 0 to ' + money(limit.max) + '.';
  if (limit.step > 1) return 'Prices in this game are stored in steps of ' + limit.step + ', from 0 to ' + money(limit.max) + '.';
  return 'Prices in this game go from 0 to ' + money(limit.max) + '.';
}

/** What is wrong with a price, or `''` when the game can store it. */
export function priceProblem(name: string, price: number, limit: PriceFormat, tm = false): string {
  if (price > limit.max) return name + ' costs ' + money(price) + '; this game stores at most ' + money(limit.max) + '.';
  if (price % limit.step) {
    const why = limit.step === 1000 ? (tm ? 'TM prices' : 'prices') + ' in this game are whole thousands.' : 'prices in this game go in steps of ' + limit.step + '.';
    return name + ' costs ' + money(price) + '; ' + why;
  }
  return '';
}

/** A typed price as a whole number of at least 0; what can't be read is 0. */
export function readPrice(text: string): number {
  const n = Math.round(Number(text.replace(/[^0-9.]/g, '')) || 0);
  return Math.min(9_999_999, Math.max(0, n));
}

/** A number typed into a field, kept within `min`–`max`; what can't be read is `min`. */
export function clampNum(text: string | number, min: number, max: number): number {
  const n = Math.round(Number(text));
  return Number.isFinite(n) && String(text).trim() !== '' ? Math.max(min, Math.min(max, n)) : min;
}
