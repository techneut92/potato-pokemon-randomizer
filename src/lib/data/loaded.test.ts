import { describe, expect, it } from 'vitest';
import { sections } from '../options/defs';
import { NOT_PORTED, OptionEngine } from '../options/engine';
import { romInfo } from './loaded';
import type { LoadedRom } from '../generated/LoadedRom';

const engine = new OptionEngine(sections());
const loaded = (generation: number, randomizable: boolean): LoadedRom => ({
  identity: { name: 'Emerald (U)', code: 'BPEE', version: 0, generation, platform: 'GameBoyAdvance', family: 'Em' },
  fileName: 'Emerald.gba',
  randomizable,
  starters: randomizable ? [252, 255, 258] : [],
  speciesCount: randomizable ? 386 : 0,
  mainGameLegendaries: false,
  staticMusicFix: randomizable,
  tweaks: randomizable ? ['fastestText', 'reusableTMs'] : [],
  palettes: randomizable,
  playerGraphicsPacks: randomizable ? [pack('may_e', 'May (Emerald)'), pack('wally', 'Wally')] : [],
  multiplePlayerCharacters: randomizable,
  capabilities: { ...NONE, ...(randomizable ? GEN3 : {}) },
  updateVersion: null
});
const NONE = { introPokemon: false, starterHeldItems: false, moveTutors: false, mapIndices: false, encounterLocations: false, shopSizes: false, timeBasedEncounters: false, megaEvolutions: false, physicalSpecialSplit: false, wildAltFormes: false, staticAltFormes: false, starterAltFormes: false, functionalFormes: false, totemPokemon: false, english: false };
const GEN3 = { introPokemon: true, starterHeldItems: true, moveTutors: true, encounterLocations: true, shopSizes: true, english: true };
function pack(id: string, name: string) {
  return { pack: { id, name, description: '', from: '', creator: '', adapter: null, category: 'pokemon' as const, romType: 'RSE' }, samples: [] };
}
const groups = (rom: ReturnType<typeof romInfo>) => engine.view(rom, {}).sections.flatMap((s) => s.groups);

describe('romInfo', () => {
  it('carries the version of a loaded game update', () => {
    expect(romInfo(loaded(3, true), '/e.gba').updateVersion).toBeUndefined();
    expect(romInfo({ ...loaded(3, true), updateVersion: 'Ver. 1.5' }, '/e.gba').updateVersion).toBe('Ver. 1.5');
  });

  it('describes a loaded Gen 3 game', () => {
    const rom = romInfo(loaded(3, true), '/games/Emerald.GBA');
    expect(rom).toMatchObject({ name: 'Pokémon Emerald (U)', meta: 'Gen 3 · Game Boy Advance', gen: 'G3', ext: '.gba', dexSize: 386, starters: ['Treecko', 'Torchic', 'Mudkip'], fileName: 'Emerald.gba' });
  });

  it('shows the options the handler says the game has', () => {
    const ruby = romInfo({ ...loaded(3, true), capabilities: { ...NONE, ...GEN3, moveTutors: false, english: false } }, '/r.gba');
    expect(ruby).toMatchObject({ tutors: false, english: false, intro: true, heldStarter: true, mapIdx: false, locs: true });
    const mt = groups(ruby).find((g) => g.key === 'mt');
    expect(mt?.unsupported).toBeTruthy();
    const md = engine.view(ruby, {}).sections.flatMap((s) => s.groups).find((g) => g.key === 'md');
    expect(md?.opts.some((o) => o.label === 'Randomize Move Names')).toBe(false);
  });

  it('offers palettes only where the handler can randomize them', () => {
    const pal = (palettes: boolean) => groups(romInfo({ ...loaded(3, true), palettes }, '/e.gba')).find((g) => g.key === 'ppal');
    expect(pal(true)?.unsupported).toBeFalsy();
    expect(pal(false)?.unsupported).toBeTruthy();
    const platinum = { ...loaded(4, true), identity: { name: 'Platinum (E)', code: 'CPUP', version: 0, generation: 4, platform: 'NintendoDs' as const, family: 'Plat' } };
    const gen4 = groups(romInfo(platinum, '/pt.nds')).find((g) => g.key === 'ppal');
    expect(gen4?.unsupported).toBeFalsy();
    expect(gen4?.opts.map((o) => o.label)).toContain('Shiny From Normal');
  });

  it('offers player graphics packs, the first by default', () => {
    const rom = romInfo(loaded(3, true), '/e.gba');
    expect(rom.cpg).toBe(true);
    expect(engine.view(rom, {}).val['cpg.pack']).toBe('may_e');
    expect(engine.view(rom, { 'cpg.pack': 'wally' }).val['cpg.pack']).toBe('wally');
    expect(engine.view(rom, { 'cpg.pack': 'gone' }).val['cpg.pack']).toBe('may_e');
    const none = romInfo({ ...loaded(3, true), playerGraphicsPacks: [] }, '/e.gba');
    expect(groups(none).find((g) => g.key === 'cpg')?.unsupported).toBeTruthy();
  });

  it('offers the second player character only in games that have one', () => {
    const who = (multiplePlayerCharacters: boolean) => {
      const rom = romInfo({ ...loaded(1, true), multiplePlayerCharacters }, '/y.gbc');
      const view = engine.view(rom, { cpg: 1, 'cpg.who.v': 1 });
      const opt = view.sections.flatMap((s) => s.groups).find((g) => g.key === 'cpg')?.opts[0];
      return { choices: opt?.choices?.map((c) => c.disabled), value: view.val['cpg.who.v'] };
    };
    expect(who(true)).toEqual({ choices: [false, false], value: 1 });
    expect(who(false)).toEqual({ choices: [false, true], value: 0 });
  });

  it('offers the misc tweaks the game supports', () => {
    const tw = groups(romInfo(loaded(3, true), '/e.gba')).find((g) => g.key === 'tw');
    expect(tw?.tweaks?.map((t) => t.label)).toEqual(['Fastest Text', 'Reusable TMs']);
  });

  it('offers No Irregular Alt Formes from Gen 4 on', () => {
    const irr = (gen: number) =>
      engine.view(romInfo(loaded(gen, true), '/g.nds'), {}).sections.flatMap((s) => s.groups).find((g) => g.key === 'gen')
        ?.opts.find((o) => o.label === 'No Irregular Alt Formes');
    expect(irr(3)).toBeUndefined();
    expect(irr(4)?.disabled).toBe(false);
    expect(irr(5)?.disabled).toBe(false);
    expect(irr(7)?.disabled).toBe(false);
  });

  it('offers Random Shiny Trainer Pokémon in Gen 7 only, which stores the flag', () => {
    const shiny = (gen: number) =>
      engine.view(romInfo(loaded(gen, true), '/g.3ds'), { tp: 1 }).sections.flatMap((s) => s.groups).find((g) => g.key === 'tp')
        ?.opts.find((o) => o.label === 'Random Shiny Trainer Pokémon');
    expect(shiny(6)).toBeUndefined();
    expect(shiny(7)?.disabled).toBe(false);
  });

  it('offers Follow Mega Evolutions for base stats, types and abilities in games with Mega Evolutions', () => {
    const follow = (gen: number, megaEvolutions: boolean) => {
      const rom = romInfo({ ...loaded(gen, true), capabilities: { ...NONE, ...GEN3, megaEvolutions } }, '/g.3ds');
      return engine.view(rom, { pbs: 2, pt: 1, pa: 1 }).sections.flatMap((s) => s.groups)
        .filter((g) => ['pbs', 'pt', 'pa'].includes(g.key))
        .map((g) => g.opts.find((o) => o.label === 'Follow Mega Evolutions')?.disabled);
    };
    expect(follow(6, true)).toEqual([false, false, false]);
    expect(follow(7, true)).toEqual([false, false, false]);
    expect(follow(5, false)).toEqual([undefined, undefined, undefined]);
  });

  it('offers Use Time Based Encounters in Gen 7 only, whose wild encounters by time of day are ported', () => {
    const time = (gen: number) => {
      const rom = { ...loaded(gen, true), capabilities: { ...NONE, ...GEN3, timeBasedEncounters: true } };
      return engine.view(romInfo(rom, '/g.3ds'), { wp: 1 }).sections.flatMap((s) => s.groups).find((g) => g.key === 'wp')
        ?.opts.find((o) => o.label === 'Use Time Based Encounters');
    };
    expect(time(7)?.disabled).toBe(false);
    expect(time(4)?.disabled).toBe(true);
    expect(time(2)?.disabled).toBe(true);
  });

  it('offers only the ported options, and says why the rest is off', () => {
    const all = groups(romInfo(loaded(3, true), '/e.gba'));
    const ported = all.filter((g) => !g.unsupported).map((g) => g.key);
    expect(ported).toEqual(['gen', 'pbs', 'pbst', 'pt', 'pa', 'pe', 'exp', 'sp', 'stp', 'igt', 'md', 'pms', 'tp', 'bs', 'wp', 'tm', 'thc', 'mt', 'mtc', 'fi', 'sh', 'pu', 'te', 'ppal', 'cpg', 'tw']);
    expect(all.filter((g) => g.unsupported).every((g) => g.note === NOT_PORTED)).toBe(true);
    // Wild held items are ported; Ban Bad Items waits for them.
    const wild = engine.view(romInfo(loaded(3, true), '/e.gba'), { wp: 1 }).sections.flatMap((s) => s.groups).find((g) => g.key === 'wp');
    expect(wild?.opts.find((o) => o.label === 'Randomize Held Items')?.disabled).toBe(false);
    expect(wild?.opts.find((o) => o.label === 'Ban Bad Items')?.disabled).toBe(true);
    expect(wild?.opts.find((o) => o.label === 'Similar Strength')?.disabled).toBe(false);
    // Metronome Only Mode is ported too.
    const movesets = engine.view(romInfo(loaded(3, true), '/e.gba'), {}).sections.flatMap((s) => s.groups).find((g) => g.key === 'pms');
    expect(movesets?.modes.map((m) => m.disabled)).toEqual([false, false, false, false]);
    // English games get random move names.
    const moves = engine.view(romInfo(loaded(3, true), '/e.gba'), {}).sections.flatMap((s) => s.groups).find((g) => g.key === 'md');
    expect(moves?.opts.find((o) => o.label === 'Randomize Move Names')?.disabled).toBe(false);
    expect(moves?.opts.find((o) => o.label === 'Randomize Move Power')?.disabled).toBe(false);
    // In a random mode every restriction is on, the type triangle included.
    const random = engine.view(romInfo(loaded(3, true), '/e.gba'), { sp: 2 }).sections.flatMap((s) => s.groups).find((g) => g.key === 'sp');
    const choices = random?.opts.find((o) => o.label === 'Type restrictions')?.choices ?? [];
    expect(choices.filter((c) => c.disabled).map((c) => c.label)).toEqual([]);
    expect(random?.opts.find((o) => o.label === 'Don’t Use Legendaries')?.disabled).toBe(false);
  });

  it('shows the static options the game supports', () => {
    const view = (rom: LoadedRom) => engine.view(romInfo(rom, '/r.gba'), { stp: 3 }).sections.flatMap((s) => s.groups).find((g) => g.key === 'stp');
    const label = (g: ReturnType<typeof view>, l: string) => g?.opts.find((o) => o.label === l);
    // Hidden options are left out of the view.
    const emerald = view(loaded(3, true));
    expect(label(emerald, 'Fix Music')?.disabled).toBe(false);
    expect(label(emerald, 'Limit Main-Game Legendaries')).toBeUndefined();
    const ruby = view({ ...loaded(3, true), mainGameLegendaries: true, staticMusicFix: false });
    expect(label(ruby, 'Limit Main-Game Legendaries')?.disabled).toBe(false);
    expect(label(ruby, 'Fix Music')).toBeUndefined();
  });

  it('turns everything off for generations that are not ported', () => {
    expect(groups(romInfo(loaded(4, false), '/p.nds')).every((g) => g.unsupported)).toBe(true);
  });
});
