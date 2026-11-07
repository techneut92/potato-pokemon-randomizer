// The manual edits of each ROM the user opened in this session, keyed by the ROM's path.
import type { Edit } from '../generated/Edit';
import type { EditGame } from '../generated/EditGame';
import type { EditList } from '../generated/EditList';
import type { ObjectRef } from '../generated/ObjectRef';
import { refKey, sameRef, sameValue, targetOf, type EditKind } from './sections';

/** The edit list format this build writes; keep in step with `EDIT_LIST_VERSION` in `ppr-model`. */
export const EDIT_LIST_VERSION = 1;

export class EditStore {
  /** Edits per ROM path, at most one per object, in the order they were first made. */
  #byRom = $state<Record<string, Edit[]>>({});
  #onChange: () => void;
  /** The path of the loaded ROM, whose edits the other members see. */
  rom = $state('');

  readonly edits: Edit[] = $derived(this.#byRom[this.rom] ?? []);
  readonly count: number = $derived(this.edits.length);
  readonly #index: Map<string, Edit> = $derived(new Map(this.edits.map((e) => [refKey(targetOf(e)), e])));

  /** `onChange` runs after every change to the loaded ROM's edits, e.g. to validate them again. */
  constructor(onChange: () => void = () => {}) {
    this.#onChange = onChange;
  }

  /** The edit of `ref`, if it has one. */
  get(ref: ObjectRef): Edit | undefined {
    return this.#index.get(refKey(ref));
  }
  edited(ref: ObjectRef): boolean {
    return this.#index.has(refKey(ref));
  }
  /** How many edits are of one of `kinds`. */
  countOf(kinds: readonly EditKind[]): number {
    return this.edits.filter((e) => kinds.includes(e.kind)).length;
  }

  /**
   * Edits an object: replaces its edit, or adds one. When `original` is given and `edit` equals it, the object is
   * back to what the game has, so its edit goes instead.
   */
  set(edit: Edit, original?: Edit) {
    const ref = targetOf(edit);
    if (original && sameValue(edit, original)) return this.revert(ref);
    this.#put(this.edited(ref) ? this.edits.map((e) => (sameRef(targetOf(e), ref) ? edit : e)) : [...this.edits, edit]);
  }
  revert(ref: ObjectRef) {
    if (this.edited(ref)) this.#put(this.edits.filter((e) => !sameRef(targetOf(e), ref)));
  }
  /** Reverts the edits of `kinds`, e.g. one section's. */
  revertKinds(kinds: readonly EditKind[]) {
    if (this.countOf(kinds)) this.#put(this.edits.filter((e) => !kinds.includes(e.kind)));
  }
  revertAll() {
    if (this.count) this.#put([]);
  }
  /**
   * Replaces the loaded ROM's edits with `edits`, e.g. a loaded edits file's. A list a person changed by hand can hold
   * two edits of one object; the later one wins, in the place of the first.
   */
  replace(edits: readonly Edit[]) {
    const byRef = new Map<string, Edit>();
    for (const e of edits) byRef.set(refKey(targetOf(e)), e);
    this.#put([...byRef.values()]);
  }
  /** Drops the edits of the ROM at `path`, e.g. when it is unloaded. */
  forget(path: string) {
    if (!(path in this.#byRom)) return;
    const rest = { ...this.#byRom };
    delete rest[path];
    this.#byRom = rest;
    if (path === this.rom) this.#onChange();
  }

  /** The loaded ROM's edits as the list Rust validates and applies. */
  list(game: EditGame): EditList {
    return { version: EDIT_LIST_VERSION, game, edits: $state.snapshot(this.edits) as Edit[] };
  }

  #put(edits: Edit[]) {
    this.#byRom = { ...this.#byRom, [this.rom]: edits };
    this.#onChange();
  }
}
