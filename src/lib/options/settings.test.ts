import { describe, expect, it } from 'vitest';
import { sampleRoms } from '../data/roms';
import { PORTED, portedFor } from '../data/loaded';
import { DEX } from '../data/dex';
import { sections } from './defs';
import { choiceList, OptionEngine, optKey } from './engine';
import { dexNumber, fromSettings, toSettings } from './settings';
import type { Overrides } from './types';

const engine = new OptionEngine(sections());
const maye = { id: 'may_e', name: 'May (Emerald)', description: '', from: '', creator: '', adapter: null, category: 'pokemon' as const, romType: 'RSE' };
const emerald = { ...sampleRoms()[1], cpgPacks: [{ pack: maye, samples: [] }] };
const settings = (ov: Overrides) => toSettings(engine.view(emerald, ov).val, emerald.genNum);

describe('toSettings', () => {
  it('leaves starters unchanged by default', () => {
    expect(settings({}).starters).toEqual({ mode: 'Unchanged', custom: [], types: 'None', noLegendaries: false, noDualTypes: false, bstMin: 0, bstMax: 0, randomHeldItems: false, banBadItems: false, allowAltFormes: false });
  });

  it('maps starter held items, whatever the starter mode', () => {
    expect(settings({ sp: 0, 'sp.held.on': true, 'sp.ban.on': true }).starters).toMatchObject({ mode: 'Unchanged', randomHeldItems: true, banBadItems: true });
    // Ban Bad Items is off without held items.
    expect(settings({ sp: 0, 'sp.ban.on': true }).starters).toMatchObject({ randomHeldItems: false, banBadItems: false });
  });

  it('maps custom starters by name, with Random as an open slot', () => {
    const s = settings({ sp: 1, 'sp.s1.v': 'Eevee', 'sp.s2.v': 'random', 'sp.s3.v': 'Mudkip' }).starters;
    expect(s.mode).toBe('Custom');
    expect(s.custom).toEqual([133, null, 258]);
  });

  it('maps random starters with their restrictions', () => {
    const s = settings({ sp: 3, 'sp.type.v': 3, 'sp.single.v': 10, 'sp.noleg.on': true, 'sp.bmin.on': true, 'sp.bmin.v': 400 }).starters;
    expect(s).toMatchObject({ mode: 'RandomWithTwoEvolutions', types: { SingleType: 'Fire' }, noLegendaries: true, bstMin: 400, bstMax: 0 });
  });

  it('sends a random single type as null', () => {
    expect(settings({ sp: 2, 'sp.type.v': 3 }).starters.types).toEqual({ SingleType: null });
  });

  it('ignores restrictions the engine disabled', () => {
    // Unchanged starters: the restrictions are disabled, so their stored values must not leak through.
    expect(settings({ sp: 0, 'sp.noleg.on': true, 'sp.type.v': 1 }).starters).toMatchObject({ noLegendaries: false, types: 'None' });
  });

  it('leaves wild Pokémon unchanged by default', () => {
    expect(settings({}).wild).toMatchObject({ mode: 'Unchanged', zones: 'Game', minCatchRate: 0, levelModifier: 0 });
  });

  it('maps wild Pokémon options', () => {
    const w = settings({ wp: 1, 'wp.zone.v': 3, 'wp.split.on': true, 'wp.tr.v': 1, 'wp.er.v': 2, 'wp.rel.on': true, 'wp.sim.on': true, 'wp.bal.on': true, 'wp.noleg.on': true, 'wp.catch.on': true, 'wp.catch.v': 4, 'wp.lvl.on': true, 'wp.lvl.v': -20 }).wild;
    expect(w).toEqual({ mode: 'Random', zones: 'Location', splitByKind: true, types: 'ZoneThemes', keepThemes: false, evolution: 'SameStage', keepRelations: true, similarStrength: true, balanceLowLevels: true, catchEmAll: false, noLegendaries: true, minCatchRate: 4, levelModifier: -20, randomHeldItems: false, banBadHeldItems: false, timeOfDay: false, allowAltFormes: false });
  });

  it('maps Pokémon palette options only when palettes are random', () => {
    expect(settings({ ppal: 1, 'ppal.types.on': true, 'ppal.shiny.on': true }).palettes).toEqual({ random: true, followTypes: true, followEvolutions: false, shinyFromNormal: true });
    expect(settings({ 'ppal.types.on': true }).palettes).toEqual({ random: false, followTypes: false, followEvolutions: false, shinyFromNormal: false });
  });

  it('maps custom player graphics only when the group is on Custom', () => {
    expect(settings({ cpg: 1, 'cpg.pack': 'may_e', 'cpg.who.v': 1 }).playerGraphics).toEqual({ pack: 'may_e', replace: 'pc2' });
    expect(settings({ cpg: 1, 'cpg.pack': 'may_e' }).playerGraphics).toEqual({ pack: 'may_e', replace: 'pc1' });
    expect(settings({ cpg: 0, 'cpg.pack': 'may_e', 'cpg.who.v': 1 }).playerGraphics).toEqual({ pack: null, replace: 'pc1' });
  });

  it('maps time-based encounters where the game has them, also without randomizing the Pokémon', () => {
    const ultraMoon = sampleRoms()[2];
    const wild = (ov: Overrides) => toSettings(engine.view(ultraMoon, ov).val, ultraMoon.genNum).wild;
    expect(wild({ wp: 1 }).timeOfDay).toBe(true);
    expect(wild({ wp: 1, 'wp.time.on': false }).timeOfDay).toBe(false);
    // Like the original, the option is on while it can't be chosen: the level modifier sees both times of day.
    expect(wild({ wp: 0, 'wp.time.on': false }).timeOfDay).toBe(true);
    // Emerald has no time-based encounters.
    expect(settings({ wp: 1, 'wp.time.on': true }).wild.timeOfDay).toBe(false);
    const back = fromSettings(toSettings(engine.view(ultraMoon, { wp: 1, 'wp.time.on': false }).val, ultraMoon.genNum), ultraMoon);
    expect(back.ov['wp.time.on']).toBe(false);
  });

  it('maps wild held items, also without randomizing the Pokémon', () => {
    expect(settings({ 'wp.held.on': true, 'wp.ban.on': true }).wild).toMatchObject({ mode: 'Unchanged', randomHeldItems: true, banBadHeldItems: true });
    expect(settings({ 'wp.ban.on': true }).wild.banBadHeldItems).toBe(false);
  });

  it('keeps wild catch rates and levels without randomizing', () => {
    expect(settings({ 'wp.catch.on': true, 'wp.catch.v': 2, 'wp.lvl.on': true, 'wp.lvl.v': 30, 'wp.sim.on': true }).wild).toMatchObject({ mode: 'Unchanged', minCatchRate: 2, levelModifier: 30, similarStrength: false });
  });

  it('never sends zone themes for a single zone', () => {
    expect(settings({ wp: 1, 'wp.zone.v': 4, 'wp.tr.v': 1 }).wild.types).not.toBe('ZoneThemes');
  });

  it('leaves static Pokémon unchanged by default', () => {
    expect(settings({}).statics).toEqual({ mode: 'Unchanged', limitMainGameLegendaries: false, randomize600Bst: false, fixMusic: false, levelModifier: 0, allowAltFormes: false, swapMegaEvos: false });
  });

  it('maps static Pokémon options', () => {
    // Ruby names a main-game legendary; Emerald doesn't, so the option is hidden there.
    const ruby = { ...emerald, mainLeg: true };
    const s = toSettings(engine.view(ruby, { stp: 3, 'stp.lim.on': true, 'stp.bst.on': true, 'stp.music.on': true, 'stp.lvl.on': true, 'stp.lvl.v': 25 }).val, ruby.genNum).statics;
    expect(s).toEqual({ mode: 'SimilarStrength', limitMainGameLegendaries: true, randomize600Bst: true, fixMusic: true, levelModifier: 25, allowAltFormes: false, swapMegaEvos: false });
  });

  it('maps Swap Mega Evolvables for Gen 6 statics', () => {
    const x = { ...emerald, genNum: 6, mega: true };
    const s = toSettings(engine.view(x, { stp: 2, 'stp.mega.on': true }).val, x.genNum).statics;
    expect(s.swapMegaEvos).toBe(true);
  });

  it('never sends a hidden option', () => {
    expect(settings({ stp: 3, 'stp.lim.on': true }).statics.limitMainGameLegendaries).toBe(false);
  });

  it('only sends similar-strength limits with similar strength, and levels without randomizing', () => {
    const s = settings({ stp: 1, 'stp.lim.on': true, 'stp.bst.on': true, 'stp.lvl.on': true, 'stp.lvl.v': -10 }).statics;
    expect(s).toMatchObject({ mode: 'RandomMatching', limitMainGameLegendaries: false, randomize600Bst: false });
    expect(settings({ 'stp.music.on': true, 'stp.lvl.on': true, 'stp.lvl.v': -10 }).statics).toMatchObject({ mode: 'Unchanged', fixMusic: false, levelModifier: -10 });
  });

  it('leaves Totem Pokémon unchanged by default and in games without them', () => {
    const none = { mode: 'Unchanged', allyMode: 'Unchanged', auraMode: 'Unchanged', randomizeHeldItems: false, allowAltFormes: false, levelModifier: 0 };
    expect(settings({}).totems).toEqual(none);
    expect(settings({ totp: 2, 'totp.ally.v': 1, 'totp.held.on': true }).totems).toEqual(none);
  });

  it('maps Totem Pokémon options, whatever the totem mode', () => {
    const ultraMoon = { ...sampleRoms()[2], ported: PORTED };
    const totems = (ov: Overrides) => toSettings(engine.view(ultraMoon, ov).val, ultraMoon.genNum).totems;
    expect(totems({ totp: 2, 'totp.ally.v': 1, 'totp.aura.v': 2, 'totp.held.on': true, 'totp.lvl.on': true, 'totp.lvl.v': -20 })).toEqual({
      mode: 'SimilarStrength',
      allyMode: 'Random',
      auraMode: 'SameStrength',
      randomizeHeldItems: true,
      allowAltFormes: false,
      levelModifier: -20
    });
    expect(totems({ 'totp.ally.v': 1, 'totp.alt.on': true }).allowAltFormes).toBe(true);
    expect(totems({ 'totp.ally.v': 2, 'totp.aura.v': 1 })).toMatchObject({ mode: 'Unchanged', allyMode: 'SimilarStrength', auraMode: 'Random' });
    // Every option is ported, alternate formes included.
    const group = engine.view(ultraMoon, { totp: 1 }).sections.flatMap((s) => s.groups).find((g) => g.key === 'totp');
    expect(group?.unsupported).toBe(false);
    const disabled = (label: string) => group?.opts.find((o) => o.label === label)?.disabled;
    expect(['Ally Pokémon', 'Auras', 'Randomize Held Items', 'Allow Alternate Formes', 'Percentage Level Modifier'].map(disabled)).toEqual([false, false, false, false, false]);
  });

  it('maps base statistics', () => {
    expect(settings({}).baseStats).toEqual({ mode: 'Unchanged', followEvolutions: false, followMegas: false, randomEvolutionStats: false, updateToGeneration: null });
    expect(settings({ pbs: 2, 'pbs.fe.on': true, 'pbs.ae.on': true }).baseStats).toMatchObject({ mode: 'Random', followEvolutions: true, randomEvolutionStats: true });
    expect(settings({ pbs: 1, 'pbs.fe.on': true, 'pbs.ae.on': true }).baseStats).toMatchObject({ mode: 'Shuffle', followEvolutions: true, randomEvolutionStats: false });
    expect(settings({ pbs: 2, 'pbs.ae.on': true }).baseStats.randomEvolutionStats).toBe(false);
    expect(settings({ 'pbs.fe.on': true }).baseStats.followEvolutions).toBe(false);
    // Gen 3's choices are Gen 6 to 9; updating works without randomizing.
    expect(settings({ 'pbs.upd.on': true, 'pbs.upd.v': 1 }).baseStats.updateToGeneration).toBe(7);
    expect(settings({ 'pbs.upd.on': true }).baseStats.updateToGeneration).toBe(6);
  });

  it('maps Follow Mega Evolutions in games that have them', () => {
    const ultraMoon = { ...sampleRoms()[2], ported: PORTED };
    const mega = (ov: Overrides) => toSettings(engine.view(ultraMoon, ov).val, ultraMoon.genNum);
    const on = { 'pbs.fm.on': true, 'pt.fm.on': true, 'pa.fm.on': true };
    const s = mega({ pbs: 2, pt: 1, pa: 1, 'pbs.ae.on': true, ...on });
    // As in the original, following Mega Evolutions alone allows random added stats.
    expect(s.baseStats).toMatchObject({ followEvolutions: false, followMegas: true, randomEvolutionStats: true });
    expect(s.types.followMegas).toBe(true);
    expect(s.abilities.followMegas).toBe(true);
    const back = fromSettings(s, ultraMoon);
    expect(toSettings(engine.view(ultraMoon, back.ov).val, ultraMoon.genNum)).toMatchObject({ baseStats: s.baseStats, types: s.types, abilities: s.abilities });
    // Off while their group is unchanged, and where alternate formes evolve at random without No Irregular Alt Formes.
    const off = mega(on);
    expect([off.baseStats.followMegas, off.types.followMegas, off.abilities.followMegas]).toEqual([false, false, false]);
    const formes = sampleRoms()[2];
    const evolving = toSettings(engine.view(formes, { pbs: 2, pt: 1, pa: 1, pe: 2, 'pe.alt.on': true, ...on }).val, formes.genNum);
    expect(evolving.abilities.followMegas).toBe(false);
    // Emerald has no Mega Evolutions.
    expect(settings({ pbs: 2, pt: 1, pa: 1, ...on }).baseStats.followMegas).toBe(false);
  });

  it('maps base stat totals', () => {
    expect(settings({}).bsts).toEqual({ mode: 'Unchanged', maxBuffNerfPercent: 0, followEvolutions: false, separateLegendaries: false });
    expect(settings({ pbst: 1, 'pbst.bn.v': 25, 'pbst.fe.on': true, 'pbst.sl.on': true }).bsts).toEqual({ mode: 'BuffNerf', maxBuffNerfPercent: 25, followEvolutions: true, separateLegendaries: false });
    expect(settings({ pbst: 2, 'pbst.bn.v': 25, 'pbst.fe.on': true, 'pbst.sl.on': true }).bsts).toEqual({ mode: 'Shuffle', maxBuffNerfPercent: 0, followEvolutions: true, separateLegendaries: true });
    expect(settings({ pbst: 3, 'pbst.fe.on': true }).bsts).toEqual({ mode: 'Random', maxBuffNerfPercent: 0, followEvolutions: false, separateLegendaries: false });
  });

  it('maps types', () => {
    expect(settings({}).types).toEqual({ mode: 'Unchanged', forceDual: false, followMegas: false });
    expect(settings({ pt: 1, 'pt.dual.on': true }).types).toEqual({ mode: 'FollowEvolutions', forceDual: true, followMegas: false });
    expect(settings({ pt: 2 }).types).toEqual({ mode: 'Random', forceDual: false, followMegas: false });
  });

  it('maps abilities', () => {
    const none = { mode: 'Unchanged', allowWonderGuard: false, followEvolutions: false, followMegas: false, combineDuplicates: false, ensureTwo: false, banTrapping: false, banNegative: false, banBad: false };
    expect(settings({ 'pa.wg.on': true }).abilities).toEqual(none);
    const all = { 'pa.wg.on': true, 'pa.fe.on': true, 'pa.dup.on': true, 'pa.two.on': true, 'pa.trap.on': true, 'pa.neg.on': true, 'pa.bad.on': true };
    expect(settings({ pa: 1, ...all }).abilities).toEqual({ mode: 'Random', allowWonderGuard: true, followEvolutions: true, followMegas: false, combineDuplicates: true, ensureTwo: true, banTrapping: true, banNegative: true, banBad: true });
  });

  it('maps move data', () => {
    expect(settings({}).moveData).toEqual({ power: false, accuracy: false, pp: false, types: false, category: false, names: false, updateToGeneration: null });
    const all = { 'md.pow.on': true, 'md.acc.on': true, 'md.pp.on': true, 'md.type.on': true, 'md.name.on': true, 'md.upd.on': true, 'md.upd.v': 2 };
    expect(settings(all).moveData).toEqual({ power: true, accuracy: true, pp: true, types: true, category: false, names: true, updateToGeneration: 6 });
  });

  it('maps movesets, with options only in a random mode', () => {
    const opts = { 'pms.l1.on': true, 'pms.l1.v': 3, 'pms.ord.on': true, 'pms.ngb.on': true, 'pms.good.on': true, 'pms.good.v': 40 };
    expect(settings({ pms: 1, ...opts }).movesets).toEqual({ mode: 'RandomPreferSameType', guaranteedLevelOneMoves: 3, reorderDamaging: true, noGameBreaking: true, goodDamagingPercent: 40, evolutionMovesForAll: false });
    expect(settings({ pms: 2 }).movesets).toMatchObject({ mode: 'RandomCompletely', guaranteedLevelOneMoves: null, goodDamagingPercent: null });
    expect(settings({ pms: 0, ...opts }).movesets).toEqual({ mode: 'Unchanged', guaranteedLevelOneMoves: null, reorderDamaging: false, noGameBreaking: false, goodDamagingPercent: null, evolutionMovesForAll: false });
    expect(settings({ pms: 3, ...opts }).movesets).toEqual({ mode: 'MetronomeOnly', guaranteedLevelOneMoves: null, reorderDamaging: false, noGameBreaking: false, goodDamagingPercent: null, evolutionMovesForAll: false });
  });

  it('maps TM moves', () => {
    const opts = { 'tm.field.on': true, 'tm.ngb.on': true, 'tm.good.on': true, 'tm.good.v': 25 };
    expect(settings({ tm: 1, ...opts }).tms).toEqual({ random: true, keepFieldMoves: true, noGameBreaking: true, goodDamagingPercent: 25 });
    expect(settings({ tm: 0, ...opts }).tms).toEqual({ random: false, keepFieldMoves: false, noGameBreaking: false, goodDamagingPercent: null });
  });

  it('maps TM/HM compatibility', () => {
    const opts = { 'thc.san.on': true, 'thc.fe.on': true, 'thc.hm.on': true };
    expect(settings({ thc: 1, ...opts }).tmHmCompat).toEqual({ mode: 'RandomPreferSameType', levelUpSanity: true, followEvolutions: true, fullHm: true });
    expect(settings({ thc: 3, ...opts }).tmHmCompat).toEqual({ mode: 'Full', levelUpSanity: true, followEvolutions: true, fullHm: false });
    expect(settings({ thc: 0, 'thc.fe.on': true }).tmHmCompat).toEqual({ mode: 'Unchanged', levelUpSanity: false, followEvolutions: false, fullHm: false });
  });

  it('maps type effectiveness', () => {
    expect(settings({ te: 4, 'te.imm.on': true, 'te.upd.on': true }).typeEffectiveness).toEqual({ mode: 'Inverse', randomImmunities: true, update: true });
    expect(settings({ te: 2, 'te.imm.on': true }).typeEffectiveness).toEqual({ mode: 'RandomBalanced', randomImmunities: false, update: false });
  });

  it('maps trainer Pokémon', () => {
    const none = { rivalCarriesStarter: false, similarStrength: false, avoidDuplicates: false, weightTypes: false, noLegendaries: false, noEarlyWonderGuard: false, localPokemon: false, randomShiny: false, eliteFourUnique: null, additionalPokemon: [0, 0, 0], diverseTypes: [false, false, false], betterMovesets: [false, false, false], heldItems: [false, false, false], consumableItemsOnly: false, sensibleItemsOnly: false, highestLevelGetsItem: false, randomNames: false, randomClassNames: false, allowAltFormes: false, swapMegaEvos: false };
    expect(settings({ 'tp.lvl.on': true, 'tp.lvl.v': -20, 'tp.evo.on': true, 'tp.evo.v': 10 }).trainers).toEqual({ ...none, mode: 'Unchanged', levelModifier: -20, evolve: 10 });
    expect(settings({ tp: 1, 'tp.evo.v': 10 }).trainers).toEqual({ ...none, mode: 'Random', levelModifier: null, evolve: null });
    const all = { 'tp.rival.on': true, 'tp.sim.on': true, 'tp.dup.on': true, 'tp.weight.on': true, 'tp.noleg.on': true, 'tp.wg.on': true, 'tp.local.on': true, 'tp.shiny.on': true, 'tp.e4.on': true, 'tp.e4.v': 2, 'tp.t00.on': true, 'tp.t00.v': 3, 'tp.t02.on': true, 'tp.t02.v': 1, 'tp.t21.on': true, 'tp.t10.on': true, 'tp.t12.on': true, 'tp.sens.on': true, 'tp.high.on': true, 'tp.names.on': true, 'tp.t31.on': true };
    expect(settings({ tp: 1, ...all }).trainers).toEqual({ mode: 'Random', levelModifier: null, evolve: null, rivalCarriesStarter: true, similarStrength: true, avoidDuplicates: true, weightTypes: false, noLegendaries: true, noEarlyWonderGuard: true, localPokemon: true, randomShiny: false, eliteFourUnique: 2, additionalPokemon: [3, 0, 1], diverseTypes: [false, true, false], betterMovesets: [false, true, false], heldItems: [true, false, true], consumableItemsOnly: false, sensibleItemsOnly: true, highestLevelGetsItem: true, randomNames: true, randomClassNames: false, allowAltFormes: false, swapMegaEvos: false });
    // Emerald has no shiny trainer Pokémon; Gen 7 does.
    expect(toSettings({ tp: 1, 'tp.shiny.on': true }, 7).trainers.randomShiny).toBe(true);
  });

  it('maps Swap Mega Evolvables for trainers where the game has Mega Evolutions', () => {
    expect(settings({ tp: 1, 'tp.mega.on': true }).trainers.swapMegaEvos).toBe(false);
    const ultraMoon = { ...sampleRoms()[2], ported: PORTED };
    for (const rom of [{ ...emerald, genNum: 6, mega: true }, ultraMoon]) {
      const s = toSettings(engine.view(rom, { tp: 1, 'tp.mega.on': true }).val, rom.genNum).trainers;
      expect(s.swapMegaEvos).toBe(true);
      expect(fromSettings(toSettings(engine.view(rom, { tp: 1, 'tp.mega.on': true }).val, rom.genNum), rom).ov['tp.mega.on']).toBe(true);
      // Trainers' species have to change for it.
      expect(toSettings(engine.view(rom, { 'tp.mega.on': true }).val, rom.genNum).trainers.swapMegaEvos).toBe(false);
    }
  });

  it('maps misc tweaks the game supports', () => {
    expect(settings({}).miscTweaks).toEqual([]);
    expect(settings({ 'tw.fastestText': true, 'tw.reusableTMs': true, 'tw.bwPatch': true }).miscTweaks).toEqual(['fastestText', 'reusableTMs']);
  });

  it('maps general options', () => {
    expect(settings({}).general).toEqual({ randomIntroPokemon: true, noPrematureEvolutions: false, noIrregularAltFormes: false });
    expect(settings({ 'gen.intro.on': true, 'gen.prem.on': true }).general).toEqual({ randomIntroPokemon: false, noPrematureEvolutions: true, noIrregularAltFormes: false });
  });

  it('maps alternate formes where the game has them', () => {
    const formes = { 'gen.irr.on': true, tp: 1, 'tp.alt.on': true, wp: 1, 'wp.alt.on': true };
    expect(settings(formes).general.noIrregularAltFormes).toBe(false);
    expect(settings(formes).trainers.allowAltFormes).toBe(false);
    const black2 = { ...sampleRoms()[0], genNum: 5, wildAlt: true };
    const s = toSettings(engine.view(black2, formes).val, black2.genNum);
    expect([s.general.noIrregularAltFormes, s.trainers.allowAltFormes, s.wild.allowAltFormes]).toEqual([true, true, true]);
    const platinum = sampleRoms()[0];
    expect(toSettings(engine.view(platinum, formes).val, platinum.genNum).wild.allowAltFormes).toBe(false);
    const limit = { inc: [1, 0, 1, 1, 1, 1, 1], related: false };
    expect(toSettings(engine.view(emerald, {}).val, emerald.genNum, limit).general.limitPokemon).toBeUndefined();
    expect(toSettings(engine.view(emerald, { 'gen.limit.on': true }).val, emerald.genNum, limit).general.limitPokemon).toEqual({
      generations: [true, false, true, false, false, false, false],
      allowEvolutionaryRelatives: false
    });
  });

  it('maps battle styles', () => {
    expect(settings({ bs: 2, 'bs.style.v': 1 }).battleStyle).toEqual({ mode: 'SingleStyle', style: 'Double' });
    expect(settings({ bs: 1, 'bs.style.v': 1 }).battleStyle).toEqual({ mode: 'Random', style: 'Single' });
  });

  it('maps pickup items', () => {
    expect(settings({ pu: 1, 'pu.ban.on': true }).pickup).toEqual({ random: true, banBad: true });
    expect(settings({ pu: 0, 'pu.ban.on': true }).pickup).toEqual({ random: false, banBad: false });
  });

  it('maps shop items', () => {
    const opts = { 'sh.op.on': true, 'sh.bad.on': true, 'sh.reg.on': true, 'sh.evo.on': true, 'sh.x.on': true, 'sh.price.on': true, 'sh.candy.on': true };
    expect(settings({ sh: 2, ...opts }).shops).toEqual({ mode: 'Random', banOverpowered: true, banBad: true, banRegular: true, guaranteeEvolutionItems: true, guaranteeXItems: true, balancePrices: true, addCheapRareCandies: true });
    expect(settings({ sh: 1, ...opts }).shops).toEqual({ mode: 'Shuffle', banOverpowered: false, banBad: false, banRegular: false, guaranteeEvolutionItems: false, guaranteeXItems: false, balancePrices: true, addCheapRareCandies: true });
  });

  it('maps field items', () => {
    expect(settings({ fi: 3, 'fi.ban.on': true }).fieldItems).toEqual({ mode: 'RandomEven', banBad: true });
    expect(settings({ fi: 1, 'fi.ban.on': true }).fieldItems).toEqual({ mode: 'Shuffle', banBad: false });
  });

  it('maps move tutor compatibility', () => {
    expect(settings({ mtc: 2, 'mtc.san.on': true, 'mtc.fe.on': true }).tutorCompat).toEqual({ mode: 'RandomCompletely', levelUpSanity: true, followEvolutions: true });
    expect(settings({ mtc: 0, 'mtc.fe.on': true }).tutorCompat).toEqual({ mode: 'Unchanged', levelUpSanity: false, followEvolutions: false });
  });

  it('maps move tutor moves', () => {
    const opts = { 'mt.field.on': true, 'mt.ngb.on': true, 'mt.good.on': true, 'mt.good.v': 60 };
    expect(settings({ mt: 1, ...opts }).tutors).toEqual({ random: true, keepFieldMoves: true, noGameBreaking: true, goodDamagingPercent: 60 });
    expect(settings({ mt: 0, ...opts }).tutors).toEqual({ random: false, keepFieldMoves: false, noGameBreaking: false, goodDamagingPercent: null });
  });

  it('maps the evolution improvements', () => {
    expect(settings({}).evolutions).toMatchObject({ changeImpossible: false, easierMaxLevel: null, useEstimatedLevels: false, removeTimeBased: false });
    const all = { 'pe.ci.on': true, 'pe.easy.on': true, 'pe.easy.v': 35, 'pe.est.on': true, 'pe.time.on': true };
    expect(settings(all).evolutions).toMatchObject({ changeImpossible: true, easierMaxLevel: 35, useEstimatedLevels: true, removeTimeBased: true });
  });

  it('maps random evolutions, with each option only where its mode has it', () => {
    const opts = { 'pe.ss.on': true, 'pe.st.on': true, 'pe.l3.on': true, 'pe.fc.on': true, 'pe.fg.on': true, 'pe.nc.on': true, 'pe.alt.on': true, 'pe.adj.on': true };
    expect(settings({ pe: 1, ...opts }).evolutions).toMatchObject({ mode: 'Random', similarStrength: true, sameTyping: true, limitThreeStages: true, forceChange: true, forceGrowth: true, noConvergence: true, adjustLevels: true });
    // Allow Alternate Formes is a Gen 7 option.
    expect(settings({ pe: 1, ...opts }).evolutions.allowAltFormes).toBe(false);
    expect(settings({ pe: 2, ...opts }).evolutions).toMatchObject({ mode: 'RandomEveryLevel', similarStrength: false, sameTyping: true, limitThreeStages: false, forceChange: true, forceGrowth: false, noConvergence: true });
    expect(settings({ pe: 0, ...opts }).evolutions).toMatchObject({ mode: 'Unchanged', sameTyping: false, forceChange: false, adjustLevels: false });
    // Adjusting levels also fits new base stat totals.
    expect(settings({ pe: 0, pbst: 1, ...opts }).evolutions).toMatchObject({ mode: 'Unchanged', adjustLevels: true });
  });

  it('maps EXP curves', () => {
    expect(settings({}).expCurves).toEqual({ standardize: null, applyTo: 'Legendaries' });
    expect(settings({ 'exp.std.on': true, 'exp.std.v': 4, 'exp.who.v': 2 }).expCurves).toEqual({ standardize: 'Fast', applyTo: 'All' });
    expect(settings({ 'exp.std.on': true, 'exp.who.v': 1 }).expCurves).toEqual({ standardize: 'MediumFast', applyTo: 'StrongLegendaries' });
  });

  it('maps in-game trades', () => {
    expect(settings({}).trades).toEqual({ mode: 'Unchanged', randomNicknames: false, randomOts: false, randomIvs: false, randomItems: false });
    const t = settings({ igt: 2, 'igt.nick.on': true, 'igt.ot.on': true, 'igt.iv.on': true, 'igt.item.on': true }).trades;
    expect(t).toEqual({ mode: 'GivenAndRequested', randomNicknames: true, randomOts: true, randomIvs: true, randomItems: true });
    expect(settings({ igt: 1, 'igt.iv.on': true }).trades).toMatchObject({ mode: 'Given', randomIvs: true, randomItems: false });
  });

  it('finds species case-insensitively', () => {
    expect([dexNumber('bulbasaur'), dexNumber(' Mew '), dexNumber('Random'), dexNumber('MissingNo')]).toEqual([1, 151, null, null]);
  });
});

/** A seeded generator, so a failing case can be replayed. */
function prng(seed: number) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Random choices for every option of every group, within each option's range. */
function randomOverrides(rnd: () => number): Overrides {
  const pick = (n: number) => Math.floor(rnd() * n);
  const ov: Overrides = {};
  for (const g of sections().flatMap((s) => s.groups)) {
    if (g.modes) ov[g.key] = pick(g.modes.length);
    if (g.tweaks) for (const id of emerald.tweaks) ov['tw.' + id] = rnd() < 0.5;
    g.opts.forEach((o, oi) => {
      const k = optKey(g, o, oi);
      ov[k + '.on'] = rnd() < 0.5;
      if (o.k === 'species') ov[k + '.v'] = rnd() < 0.2 ? 'Random' : DEX[pick(emerald.dexSize)];
      else if (o.min !== undefined && o.max !== undefined) ov[k + '.v'] = o.min + pick(o.max - o.min + 1);
      else if (o.choices) ov[k + '.v'] = pick(Math.max(1, choiceList(o, emerald).length));
    });
  }
  return ov;
}

describe('fromSettings', () => {
  it('gives back the settings it was made from', () => {
    const rnd = prng(7);
    const limit = { inc: [1, 0, 1, 0, 0, 0, 0], related: false };
    for (let i = 0; i < 300; i++) {
      const before = toSettings(engine.view(emerald, randomOverrides(rnd)).val, emerald.genNum, limit);
      const loaded = fromSettings(before, emerald);
      const after = toSettings(engine.view(emerald, loaded.ov).val, emerald.genNum, loaded.limit ?? undefined);
      // Custom player graphics aren't part of settings strings and files.
      expect({ ...after, playerGraphics: null }, 'case ' + i).toEqual({ ...before, playerGraphics: null });
      expect(loaded.starterUnavailable).toBe(false);
    }
  });

  it('gives back Totem Pokémon settings in Gen 7', () => {
    const ultraMoon = { ...sampleRoms()[2], ported: PORTED };
    const ov = { totp: 1, 'totp.ally.v': 2, 'totp.aura.v': 1, 'totp.held.on': true, 'totp.alt.on': true, 'totp.lvl.on': true, 'totp.lvl.v': 30 };
    const before = toSettings(engine.view(ultraMoon, ov).val, ultraMoon.genNum);
    const loaded = fromSettings(before, ultraMoon);
    expect(before.totems.allowAltFormes).toBe(true);
    expect(toSettings(engine.view(ultraMoon, loaded.ov).val, ultraMoon.genNum).totems).toEqual(before.totems);
  });

  it('offers Allow Alternate Formes for starters and evolutions in Gen 7, and gives them back', () => {
    const ultraMoon = { ...sampleRoms()[2], ported: portedFor(7) };
    const view = engine.view(ultraMoon, { sp: 2, pe: 1, stp: 2, 'sp.alt.on': true, 'pe.alt.on': true });
    const opt = (group: string) => view.sections.flatMap((s) => s.groups).find((g) => g.key === group)?.opts.find((o) => o.label === 'Allow Alternate Formes');
    expect([opt('sp')?.disabled, opt('pe')?.disabled, opt('stp')?.disabled]).toEqual([false, false, false]);
    const before = toSettings(view.val, ultraMoon.genNum);
    expect([before.starters.allowAltFormes, before.evolutions.allowAltFormes]).toEqual([true, true]);
    const after = toSettings(engine.view(ultraMoon, fromSettings(before, ultraMoon).ov).val, ultraMoon.genNum);
    expect([after.starters.allowAltFormes, after.evolutions.allowAltFormes]).toEqual([true, true]);
  });

  it('carries Race Mode and Limit Pokémon', () => {
    const s = toSettings(engine.view(emerald, { 'gen.limit.on': true }).val, emerald.genNum, { inc: [1, 1, 0, 0, 0, 0, 0], related: true });
    const loaded = fromSettings(s, emerald, true);
    expect(loaded.ov['gen.race.on']).toBe(true);
    expect(loaded.limit).toEqual({ inc: [1, 1, 0, 0, 0, 0, 0], related: true });
  });

  it('keeps the game\'s starter when a custom one is not in the game', () => {
    const s = settings({ sp: 1, 'sp.s1.v': 'Eevee' });
    s.starters.custom = [999, 133, null];
    const loaded = fromSettings(s, emerald);
    expect(loaded.starterUnavailable).toBe(true);
    expect(loaded.ov['sp.s1.v']).toBeUndefined();
    expect(toSettings(engine.view(emerald, loaded.ov).val, emerald.genNum).starters.custom).toEqual([dexNumber(emerald.starters[0]), 133, null]);
  });
});
