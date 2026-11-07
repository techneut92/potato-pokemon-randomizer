import { describe, expect, it } from 'vitest';
import { sampleRoms, type RomInfo } from '../data/roms';
import { sections } from './defs';
import { OptionEngine, type GroupView, type View } from './engine';
import type { Overrides } from './types';

const engine = new OptionEngine(sections());
const [platinum, emerald, ultraMoon] = sampleRoms();

/** Small page-object over the engine view, so tests read like the UI. */
class Screen {
  ov: Overrides = {};
  constructor(private readonly rom: RomInfo) {}
  get view(): View {
    return engine.view(this.rom, this.ov);
  }
  group(title: string): GroupView {
    const g = this.view.sections.flatMap((s) => s.groups).find((x) => x.title === title);
    if (!g) throw new Error('No group ' + title);
    return g;
  }
  opt(group: string, label: string) {
    return this.group(group).opts.find((o) => o.label === label);
  }
  mode(group: string, label: string) {
    const g = this.group(group);
    const m = g.modes.find((x) => x.label === label);
    if (!m) throw new Error('No mode ' + label + ' in ' + group);
    this.ov = { ...this.ov, [g.key]: m.i };
    return this;
  }
  check(group: string, label: string, on = true) {
    const o = this.opt(group, label);
    if (!o) throw new Error('No option ' + label);
    this.ov = { ...this.ov, [o.key + '.on']: on };
    return this;
  }
  enabled(group: string, label: string) {
    const o = this.opt(group, label);
    return !!o && !o.disabled;
  }
}

describe('OptionEngine', () => {
  it('shows options by ROM capability', () => {
    const p = new Screen(platinum);
    expect(p.opt('General Options', 'No Irregular Alt Formes')).toBeDefined();
    expect(p.opt('Base Statistics', 'Follow Mega Evolutions')).toBeUndefined();
    expect(new Screen(ultraMoon).opt('Base Statistics', 'Follow Mega Evolutions')).toBeDefined();
    expect(p.opt('Starter Pokémon', 'Allow Alternate Formes')).toBeUndefined();
    expect(p.opt('Static Pokémon', 'Limit Main-Game Legendaries')).toBeDefined();
  });

  it('enables follow-ups only for the right base-stat mode', () => {
    const p = new Screen(platinum);
    expect(p.enabled('Base Statistics', 'Follow Evolutions')).toBe(false);
    p.mode('Base Statistics', 'Random');
    expect(p.enabled('Base Statistics', 'Follow Evolutions')).toBe(true);
    expect(p.enabled('Base Statistics', 'Randomize Added Stats on Evolution')).toBe(false);
    p.check('Base Statistics', 'Follow Evolutions');
    expect(p.enabled('Base Statistics', 'Randomize Added Stats on Evolution')).toBe(true);
  });

  it('applies the evolution-lines restrictions', () => {
    const p = new Screen(platinum).mode('Evolutions', 'Random Every Level');
    expect(p.group('Types').modes.find((m) => m.label === 'Random (follow evolutions)')?.disabled).toBe(true);
    expect(p.enabled('Evolutions', 'Change Impossible Evolutions')).toBe(false);
    expect(p.enabled('Evolutions', 'Same Typing')).toBe(true);
    expect(p.enabled('Evolutions', 'Similar Strength')).toBe(false);
  });

  it('starts every ROM without changes and with valid starters', () => {
    for (const rom of [platinum, emerald, ultraMoon]) {
      const v = engine.view(rom, {});
      expect(v.total).toBe(0);
      expect(v.badStarter).toBe(false);
    }
  });

  it('finds options by search query', () => {
    const v = engine.view(platinum, {}, 'running shoes');
    expect(v.hits.length).toBeGreaterThan(0);
  });
});
