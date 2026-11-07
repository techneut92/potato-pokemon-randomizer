// The picker dialog's model: one dialog for species, items, types, moves and abilities.

export type PickValue = string | number | null;

/** One choice: what it sets, its name, and a short tag in front (a dex number, "Original", …). */
export interface PickEntry {
  value: PickValue;
  label: string;
  num: string;
}

/** A row of filter pills, e.g. by generation; the first option shows everything. */
export interface PickFilters {
  label: string;
  options: { label: string; test: (e: PickEntry) => boolean }[];
}

export interface PickRequest {
  title: string;
  lede: string;
  /** The search field's placeholder; no search field when absent. */
  hint?: string;
  /** What the entries are, for "12 of 386 Pokémon". */
  noun: string;
  entries: PickEntry[];
  /** Shown before the entries and never filtered out: Original, Random, None. */
  specials: PickEntry[];
  filters?: PickFilters;
  /** The value the field has now, marked in the list. */
  current: PickValue;
  apply: (value: PickValue) => void;
}

/** First national dex index of each generation, plus the end of Gen 7. */
export const GEN_STARTS = [0, 151, 251, 386, 493, 649, 721, 807];

/** Pills for the generations up to `genNum`, for species entries tagged with their dex number (`#133`). */
export function generationFilters(genNum: number): PickFilters {
  const options: PickFilters['options'] = [{ label: 'All', test: () => true }];
  for (let g = 1; g <= genNum; g++) {
    options.push({
      label: 'Gen ' + g,
      test: (e: PickEntry) => {
        const n = Number(e.num.replace(/^#/, ''));
        return n > GEN_STARTS[g - 1] && n <= GEN_STARTS[g];
      }
    });
  }
  return { label: 'Generation', options };
}

/** Species entries for national numbers 1 to `names.length`; `byName` makes their value the name, else the number. */
export function speciesEntries(names: readonly string[], byName = false): PickEntry[] {
  return names.map((label, i) => ({ value: byName ? label : i + 1, label, num: '#' + String(i + 1).padStart(3, '0') }));
}

/** Whether `e` matches a search: part of its name, or its number (`133`, `#133` and `#0133` alike). */
export function matches(e: PickEntry, query: string): boolean {
  const q = query.trim().toLowerCase().replace(/^#/, '');
  if (!q) return true;
  const num = e.num.replace(/^#/, '').replace(/^0+/, '');
  return e.label.toLowerCase().includes(q) || (num !== '' && num === q.replace(/^0+/, ''));
}

/** The specials with the original value first, tagged "Original", and not listed twice. */
export function withOriginal(specials: PickEntry[], original: PickEntry | null): PickEntry[] {
  if (!original) return specials;
  return [{ ...original, num: 'Original' }, ...specials.filter((s) => String(s.value) !== String(original.value))];
}
