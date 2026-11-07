// The edits store: one edit per object, per ROM, with revert and the list Rust gets.
import { describe, expect, it, vi } from 'vitest';
import type { Edit } from '../generated/Edit';
import { EDIT_LIST_VERSION, EditStore } from './store.svelte';

const starter = (slot: number, species: number): Edit => ({ kind: 'starter', slot, species });
const shop = (index: number, items: number[]): Edit => ({ kind: 'shop', index, items });
const game = { name: 'Emerald (U)', code: 'BPEE', version: 0 };

function store() {
  const onChange = vi.fn();
  const s = new EditStore(onChange);
  s.rom = 'emerald.gba';
  return { s, onChange };
}

describe('EditStore', () => {
  it('starts empty', () => {
    const { s } = store();
    expect(s.count).toBe(0);
    expect(s.edits).toEqual([]);
    expect(s.edited({ kind: 'starter', slot: 0 })).toBe(false);
  });

  it('adds an edit and marks its object edited', () => {
    const { s, onChange } = store();
    s.set(starter(1, 258));
    expect(s.count).toBe(1);
    expect(s.edited({ kind: 'starter', slot: 1 })).toBe(true);
    expect(s.edited({ kind: 'starter', slot: 0 })).toBe(false);
    expect(s.edited({ kind: 'starterHeldItem', slot: 1 })).toBe(false);
    expect(s.get({ kind: 'starter', slot: 1 })).toEqual(starter(1, 258));
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it('keeps one edit per object, in the place it was first made', () => {
    const { s } = store();
    s.set(starter(0, 1));
    s.set(shop(3, [13]));
    s.set(starter(0, 4));
    expect(s.edits).toEqual([starter(0, 4), shop(3, [13])]);
  });

  it('drops the edit when the object is set back to its original', () => {
    const { s } = store();
    s.set(starter(0, 1), starter(0, 252));
    s.set(starter(0, 252), starter(0, 252));
    expect(s.count).toBe(0);
  });

  it('compares originals field by field, lists included', () => {
    const { s } = store();
    s.set(shop(2, [13, 14]), shop(2, [13]));
    expect(s.count).toBe(1);
    s.set(shop(2, [13]), shop(2, [13]));
    expect(s.count).toBe(0);
  });

  it('reverts one object', () => {
    const { s, onChange } = store();
    s.set(starter(0, 1));
    s.set(starter(1, 4));
    s.revert({ kind: 'starter', slot: 0 });
    expect(s.edits).toEqual([starter(1, 4)]);
    onChange.mockClear();
    s.revert({ kind: 'starter', slot: 0 });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('reverts a section’s kinds and counts per kind', () => {
    const { s } = store();
    s.set(starter(0, 1));
    s.set({ kind: 'starterHeldItem', slot: 0, item: 13 });
    s.set(shop(0, [13]));
    expect(s.countOf(['starter', 'starterHeldItem'])).toBe(2);
    s.revertKinds(['starter', 'starterHeldItem']);
    expect(s.edits).toEqual([shop(0, [13])]);
  });

  it('reverts all', () => {
    const { s } = store();
    s.set(starter(0, 1));
    s.set(shop(0, [13]));
    s.revertAll();
    expect(s.count).toBe(0);
  });

  it('keeps each ROM’s edits apart', () => {
    const { s } = store();
    s.set(starter(0, 1));
    s.rom = 'ruby.gba';
    expect(s.count).toBe(0);
    s.set(starter(0, 7));
    s.rom = 'emerald.gba';
    expect(s.edits).toEqual([starter(0, 1)]);
  });

  it('forgets an unloaded ROM’s edits', () => {
    const { s, onChange } = store();
    s.set(starter(0, 1));
    onChange.mockClear();
    s.forget('emerald.gba');
    expect(s.count).toBe(0);
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it('replaces the loaded ROM’s edits with a loaded list, and only its', () => {
    const { s, onChange } = store();
    s.set(starter(0, 1));
    s.rom = 'ruby.gba';
    s.set(starter(0, 7));
    s.rom = 'emerald.gba';
    onChange.mockClear();
    s.replace([shop(3, [13]), starter(2, 4)]);
    expect(s.edits).toEqual([shop(3, [13]), starter(2, 4)]);
    expect(s.edited({ kind: 'starter', slot: 0 })).toBe(false);
    expect(onChange).toHaveBeenCalledTimes(1);
    s.rom = 'ruby.gba';
    expect(s.edits).toEqual([starter(0, 7)]);
  });

  it('keeps one edit per object when a loaded list has two, the later winning in the first one’s place', () => {
    const { s } = store();
    s.replace([starter(0, 1), shop(3, [13]), starter(0, 4)]);
    expect(s.edits).toEqual([starter(0, 4), shop(3, [13])]);
  });

  it('replaces with an empty list too, and validates again', () => {
    const { s, onChange } = store();
    s.set(starter(0, 1));
    onChange.mockClear();
    s.replace([]);
    expect(s.count).toBe(0);
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it('builds the list Rust validates and applies, as plain data', () => {
    const { s } = store();
    s.set(shop(1, [13, 14]));
    const list = s.list(game);
    expect(list).toEqual({ version: EDIT_LIST_VERSION, game, edits: [shop(1, [13, 14])] });
    // A snapshot: later edits don't change a list already sent.
    s.set(shop(1, [15]));
    expect(list.edits).toEqual([shop(1, [13, 14])]);
    expect(JSON.parse(JSON.stringify(list))).toEqual(list);
  });
});
