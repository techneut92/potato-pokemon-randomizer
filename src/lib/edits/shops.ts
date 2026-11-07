// Shops in the editor: how each game lets a shop grow, and the shared space some games keep them in.
import type { ShopSize } from './types';

/** "1 item", "6 items". */
export function itemsText(n: number): string {
  return n + (n === 1 ? ' item' : ' items');
}

/** How shops can change size in this game, as a line under the shop; `spare` is what a shared area has free in the game. */
export function shopRuleText(size: ShopSize, generation: number, spare: number | null = null): string {
  switch (size.kind) {
    case 'fixed':
      return 'This game keeps every shop at its original size: swap items freely, but each shop must end with as many items as it started with.';
    case 'shrink':
      return 'This game has no free space to move a shop to: a shop can sell fewer items than it does now, but not more.';
    case 'grow':
      if (size.max === null) return 'A shop that grows moves to free space in the ROM, so shops can grow freely.';
      return (generation <= 3 ? 'A shop that grows moves to free space in the ROM, so shops can grow.' : 'Every shop is its own list, so shops can grow.') + ' Each shop holds at most ' + size.max + ' items.';
    case 'shared': {
      const per = [...new Set(size.costs.map((c) => c.perItem))].sort((a, b) => b - a);
      const lead = 'The shop lists share a small reserved area' + (spare === null ? ' of ' + size.bytes + ' bytes.' : ' with ' + spare + ' bytes to spare.');
      // Gen 4: the progressive shop keeps a badge requirement with each item, so its items take twice the room.
      if (per.length === 2 && size.costs[0]?.perItem === per[0])
        return lead + ' An item takes ' + per[0] + ' bytes in the progressive shop, where it keeps its badge requirement and new items are sold from the start, and ' + per[1] + ' bytes in every other counter.';
      return lead + ' Each item takes ' + (per[0] ?? 0) + ' bytes.';
    }
  }
}

/** The bytes the shops take of a shared area, with `itemsOf(i)` in shop `i`; when only changed shops count, `changed(i)` tells them. */
export function sharedUse(size: Extract<ShopSize, { kind: 'shared' }>, itemsOf: (i: number) => readonly number[], changed: (i: number) => boolean): number {
  return size.costs.reduce((n, c, i) => (size.onlyChanged && !changed(i) ? n : n + c.base + c.perItem * itemsOf(i).length), 0);
}

/** The shared area: how much there is, how much the shops take, and how much is left (below 0 when over). */
export interface ShopSpace {
  cap: number;
  used: number;
  free: number;
}

export function shopSpace(size: ShopSize, itemsOf: (i: number) => readonly number[], changed: (i: number) => boolean): ShopSpace | null {
  if (size.kind !== 'shared') return null;
  const used = sharedUse(size, itemsOf, changed);
  return { cap: size.bytes, used, free: size.bytes - used };
}

/** "200 bytes free", "over by 6 bytes". */
export function spaceText(space: ShopSpace): string {
  return space.free >= 0 ? space.free + ' bytes free' : 'over by ' + -space.free + ' bytes';
}

/** What is wrong with a shop of `n` items that has `original` in the game, or `''` when it fits. */
export function shopProblem(name: string, n: number, original: number, size: ShopSize): string {
  if (n === 0) return name + ' sells nothing. A shop needs at least one item.';
  if (size.kind === 'fixed' && n !== original) {
    const fix = n > original ? 'Remove ' + (n - original) : 'Add ' + (original - n);
    return name + ' sells ' + n + ' items, but this game keeps it at exactly ' + original + '. ' + fix + '.';
  }
  if (size.kind === 'shrink' && n > original) return name + ' sells ' + n + ' items, but this game can’t give it more than its ' + original + '. Remove ' + (n - original) + '.';
  const max = size.kind === 'grow' || size.kind === 'shared' ? size.max : null;
  if (max !== null && n > max) return name + ' sells ' + n + ' items; a shop holds at most ' + max + '.';
  return '';
}

/** The message about a shared area the shops overflow, or `''`. */
export function spaceProblem(space: ShopSpace | null): string {
  return space && space.free < 0 ? 'The shop lists need ' + -space.free + ' bytes more than the ROM has room for. Remove items from any shop.' : '';
}

/** "6 items", or "6 items of 6 (fixed)" where the game keeps the size. */
export function shopCountText(n: number, original: number, size: ShopSize): string {
  if (size.kind === 'fixed') return itemsText(n) + ' of ' + original + ' (fixed)';
  if (size.kind === 'shrink') return itemsText(n) + ' of at most ' + original;
  return itemsText(n);
}

/** Moves the item at `from` to `to` in a copy of `items`. */
export function moveItem(items: readonly number[], from: number, to: number): number[] {
  const out = [...items];
  if (to < 0 || to >= out.length) return out;
  [out[from], out[to]] = [out[to], out[from]];
  return out;
}
