import { DEX } from '../data/dex';
import type { AbilitySettings } from '../generated/AbilitySettings';
import type { BaseStatMode } from '../generated/BaseStatMode';
import type { BattleStyle } from '../generated/BattleStyle';
import type { GeneralSettings } from '../generated/GeneralSettings';
import type { GenerationLimit } from '../generated/GenerationLimit';
import type { MiscTweak } from '../generated/MiscTweak';
import { TWEAKS } from '../data/roms';
import type { BattleStyleMode } from '../generated/BattleStyleMode';
import type { BattleStyleSettings } from '../generated/BattleStyleSettings';
import type { BaseStatSettings } from '../generated/BaseStatSettings';
import type { BstMode } from '../generated/BstMode';
import type { BstSettings } from '../generated/BstSettings';
import type { EvolutionMode } from '../generated/EvolutionMode';
import type { EvolutionSettings } from '../generated/EvolutionSettings';
import type { ExpCurve } from '../generated/ExpCurve';
import type { ExpCurveScope } from '../generated/ExpCurveScope';
import type { ExpCurveSettings } from '../generated/ExpCurveSettings';
import type { MoveDataSettings } from '../generated/MoveDataSettings';
import type { CompatMode } from '../generated/CompatMode';
import type { FieldItemMode } from '../generated/FieldItemMode';
import type { FieldItemSettings } from '../generated/FieldItemSettings';
import type { MovesetMode } from '../generated/MovesetMode';
import type { MovesetSettings } from '../generated/MovesetSettings';
import type { PlayerGraphicsSettings } from '../generated/PlayerGraphicsSettings';
import type { PaletteSettings } from '../generated/PaletteSettings';
import type { PickupItemSettings } from '../generated/PickupItemSettings';
import type { Settings } from '../generated/Settings';
import type { ShopItemMode } from '../generated/ShopItemMode';
import type { ShopItemSettings } from '../generated/ShopItemSettings';
import type { StarterMode } from '../generated/StarterMode';
import type { StarterTypes } from '../generated/StarterTypes';
import type { StaticMode } from '../generated/StaticMode';
import type { StaticSettings } from '../generated/StaticSettings';
import type { TmHmCompatSettings } from '../generated/TmHmCompatSettings';
import type { TmSettings } from '../generated/TmSettings';
import type { AuraMode } from '../generated/AuraMode';
import type { TotemMode } from '../generated/TotemMode';
import type { TotemSettings } from '../generated/TotemSettings';
import type { TradeMode } from '../generated/TradeMode';
import type { TradeSettings } from '../generated/TradeSettings';
import type { TrainerMode } from '../generated/TrainerMode';
import type { TrainerSettings } from '../generated/TrainerSettings';
import type { TutorCompatSettings } from '../generated/TutorCompatSettings';
import type { TutorSettings } from '../generated/TutorSettings';
import type { Type } from '../generated/Type';
import type { TypeEffectivenessMode } from '../generated/TypeEffectivenessMode';
import type { TypeEffectivenessSettings } from '../generated/TypeEffectivenessSettings';
import type { TypeMode } from '../generated/TypeMode';
import type { TypeSettings } from '../generated/TypeSettings';
import type { WildEvolution } from '../generated/WildEvolution';
import type { WildSettings } from '../generated/WildSettings';
import type { WildTypes } from '../generated/WildTypes';
import type { WildZones } from '../generated/WildZones';
import type { Overrides } from './types';

/** Effective option values, as evaluated by the option engine (`View.val`). */
export type Values = Readonly<Record<string, unknown>>;

/** Starter modes in the order of the "Starter Pokémon" group. */
const STARTER_MODES: readonly StarterMode[] = ['Unchanged', 'Custom', 'Random', 'RandomWithTwoEvolutions', 'RandomBasic'];
/** The "Single type" choices after "Random", in the order of the engine's TYPES list. */
const TYPES: readonly Type[] = ['Normal', 'Fighting', 'Flying', 'Poison', 'Ground', 'Rock', 'Bug', 'Ghost', 'Steel', 'Fire', 'Water', 'Grass', 'Electric', 'Psychic', 'Ice', 'Dragon', 'Dark', 'Fairy'];

/** Wild zones in the order of the "Zones" choices. */
const WILD_ZONES: readonly WildZones[] = ['Slot', 'Set', 'Map', 'Location', 'Game'];
const WILD_TYPES: readonly WildTypes[] = ['None', 'ZoneThemes', 'KeepPrimary'];
const WILD_EVOLUTION: readonly WildEvolution[] = ['None', 'BasicOnly', 'SameStage'];

/** Static modes in the order of the "Static Pokémon" group. */
const STATIC_MODES: readonly StaticMode[] = ['Unchanged', 'RandomMatching', 'Random', 'SimilarStrength'];
/** Modes of the "Totem Pokémon" group and of its "Ally Pokémon" option. */
const TOTEM_MODES: readonly TotemMode[] = ['Unchanged', 'Random', 'SimilarStrength'];
/** Choices of the "Auras" option. */
const AURA_MODES: readonly AuraMode[] = ['Unchanged', 'Random', 'SameStrength'];

const num = (v: unknown) => (typeof v === 'number' ? v : Number(v) || 0);
const limit = (val: Values, key: string) => (val[key + '.on'] ? num(val[key + '.v']) : 0);
/** A toggled number, or `null` when off. */
const optional = (val: Values, key: string) => (val[key + '.on'] ? num(val[key + '.v']) : null);

/** National number of a species picked by name, or `null` for "Random". */
export function dexNumber(name: unknown): number | null {
  const i = DEX.findIndex((d) => d.toLowerCase() === String(name).trim().toLowerCase());
  return i < 0 ? null : i + 1;
}

function starterTypes(val: Values): StarterTypes {
  switch (num(val['sp.type.v'])) {
    case 1:
      return 'FireWaterGrass';
    case 2:
      return 'TypeTriangle';
    case 3: {
      const t = num(val['sp.single.v']);
      return { SingleType: t > 0 ? (TYPES[t - 1] ?? null) : null };
    }
    case 4:
      return 'Unique';
    default:
      return 'None';
  }
}

function wild(val: Values): WildSettings {
  return {
    mode: num(val['wp']) === 1 ? 'Random' : 'Unchanged',
    zones: WILD_ZONES[num(val['wp.zone.v'])] ?? 'Game',
    splitByKind: !!val['wp.split.on'],
    types: WILD_TYPES[num(val['wp.tr.v'])] ?? 'None',
    keepThemes: !!val['wp.themes.on'],
    evolution: WILD_EVOLUTION[num(val['wp.er.v'])] ?? 'None',
    keepRelations: !!val['wp.rel.on'],
    similarStrength: !!val['wp.sim.on'],
    balanceLowLevels: !!val['wp.bal.on'],
    catchEmAll: !!val['wp.cea.on'],
    noLegendaries: !!val['wp.noleg.on'],
    minCatchRate: limit(val, 'wp.catch'),
    levelModifier: limit(val, 'wp.lvl'),
    randomHeldItems: !!val['wp.held.on'],
    banBadHeldItems: !!val['wp.held.on'] && !!val['wp.ban.on'],
    timeOfDay: !!val['wp.time.on'],
    allowAltFormes: !!val['wp.alt.on']
  };
}

function statics(val: Values): StaticSettings {
  const mode = STATIC_MODES[num(val['stp'])] ?? 'Unchanged';
  const similar = mode === 'SimilarStrength';
  return {
    mode,
    limitMainGameLegendaries: similar && !!val['stp.lim.on'],
    randomize600Bst: similar && !!val['stp.bst.on'],
    fixMusic: mode !== 'Unchanged' && !!val['stp.music.on'],
    levelModifier: limit(val, 'stp.lvl'),
    allowAltFormes: !!val['stp.alt.on'],
    swapMegaEvos: !!val['stp.mega.on']
  };
}

/** Totem Pokémon: every option applies whatever the group's mode. */
function totems(val: Values): TotemSettings {
  return {
    mode: TOTEM_MODES[num(val['totp'])] ?? 'Unchanged',
    allyMode: TOTEM_MODES[num(val['totp.ally.v'])] ?? 'Unchanged',
    auraMode: AURA_MODES[num(val['totp.aura.v'])] ?? 'Unchanged',
    randomizeHeldItems: !!val['totp.held.on'],
    allowAltFormes: !!val['totp.alt.on'],
    levelModifier: limit(val, 'totp.lvl')
  };
}

/** Base stat modes in the order of the "Base Statistics" group. */
const BASE_STAT_MODES: readonly BaseStatMode[] = ['Unchanged', 'Shuffle', 'Random'];

/** The generation picked in "Update Base Stats to Generation": its choices start after the game's, at Gen 6. */
function updateTo(val: Values, genNum: number): number | null {
  return val['pbs.upd.on'] ? Math.max(6, genNum + 1) + num(val['pbs.upd.v']) : null;
}

function baseStats(val: Values, genNum: number): BaseStatSettings {
  const mode = BASE_STAT_MODES[num(val['pbs'])] ?? 'Unchanged';
  const followEvolutions = mode !== 'Unchanged' && !!val['pbs.fe.on'];
  const followMegas = mode !== 'Unchanged' && !!val['pbs.fm.on'];
  return {
    mode,
    followEvolutions,
    followMegas,
    randomEvolutionStats: mode === 'Random' && (followEvolutions || followMegas) && !!val['pbs.ae.on'],
    updateToGeneration: updateTo(val, genNum)
  };
}

/** Base stat total modes in the order of the "Base Stat Totals" group. */
const BST_MODES: readonly BstMode[] = ['Unchanged', 'BuffNerf', 'Shuffle', 'Random'];

function bsts(val: Values): BstSettings {
  const mode = BST_MODES[num(val['pbst'])] ?? 'Unchanged';
  return {
    mode,
    maxBuffNerfPercent: mode === 'BuffNerf' ? num(val['pbst.bn.v']) : 0,
    followEvolutions: (mode === 'BuffNerf' || mode === 'Shuffle') && !!val['pbst.fe.on'],
    separateLegendaries: mode === 'Shuffle' && !!val['pbst.sl.on']
  };
}

/** Type modes in the order of the "Types" group. */
const TYPE_MODES: readonly TypeMode[] = ['Unchanged', 'FollowEvolutions', 'Random'];

function types(val: Values): TypeSettings {
  const mode = TYPE_MODES[num(val['pt'])] ?? 'Unchanged';
  return { mode, forceDual: mode !== 'Unchanged' && !!val['pt.dual.on'], followMegas: mode !== 'Unchanged' && !!val['pt.fm.on'] };
}

function abilities(val: Values): AbilitySettings {
  const random = num(val['pa']) === 1;
  const on = (key: string) => random && !!val[key];
  return {
    mode: random ? 'Random' : 'Unchanged',
    allowWonderGuard: on('pa.wg.on'),
    followEvolutions: on('pa.fe.on'),
    followMegas: on('pa.fm.on'),
    combineDuplicates: on('pa.dup.on'),
    ensureTwo: on('pa.two.on'),
    banTrapping: on('pa.trap.on'),
    banNegative: on('pa.neg.on'),
    banBad: on('pa.bad.on')
  };
}

/** Curves in the order of "Standardize EXP Curves to", and the "Apply to" choices. */
const EXP_CURVES: readonly ExpCurve[] = ['MediumFast', 'Erratic', 'Fluctuating', 'MediumSlow', 'Fast', 'Slow'];
const EXP_SCOPES: readonly ExpCurveScope[] = ['Legendaries', 'StrongLegendaries', 'All'];

function expCurves(val: Values): ExpCurveSettings {
  return {
    standardize: val['exp.std.on'] ? (EXP_CURVES[num(val['exp.std.v'])] ?? 'MediumFast') : null,
    applyTo: EXP_SCOPES[num(val['exp.who.v'])] ?? 'Legendaries'
  };
}

/** Evolution modes in the order of the "Evolutions" group. */
const EVOLUTION_MODES: readonly EvolutionMode[] = ['Unchanged', 'Random', 'RandomEveryLevel'];

function evolutions(val: Values): EvolutionSettings {
  const mode = EVOLUTION_MODES[num(val['pe'])] ?? 'Unchanged';
  const random = (key: string) => mode === 'Random' && !!val[key];
  const any = (key: string) => mode !== 'Unchanged' && !!val[key];
  return {
    mode,
    similarStrength: random('pe.ss.on'),
    sameTyping: any('pe.st.on'),
    limitThreeStages: random('pe.l3.on'),
    forceChange: any('pe.fc.on'),
    forceGrowth: random('pe.fg.on'),
    noConvergence: any('pe.nc.on'),
    allowAltFormes: any('pe.alt.on'),
    adjustLevels: !!val['pe.adj.on'],
    changeImpossible: !!val['pe.ci.on'],
    easierMaxLevel: val['pe.easy.on'] ? num(val['pe.easy.v']) : null,
    useEstimatedLevels: !!val['pe.est.on'],
    removeTimeBased: !!val['pe.time.on']
  };
}

function moveData(val: Values, genNum: number): MoveDataSettings {
  return {
    power: !!val['md.pow.on'],
    accuracy: !!val['md.acc.on'],
    pp: !!val['md.pp.on'],
    types: !!val['md.type.on'],
    category: !!val['md.cat.on'],
    names: !!val['md.name.on'],
    // The choices start at the next generation.
    updateToGeneration: val['md.upd.on'] ? genNum + 1 + num(val['md.upd.v']) : null
  };
}

/** Moveset modes in the order of the "Pokémon Movesets" group; Metronome Only Mode is not ported yet. */
const MOVESET_MODES: readonly MovesetMode[] = ['Unchanged', 'RandomPreferSameType', 'RandomCompletely', 'MetronomeOnly'];

function movesets(val: Values): MovesetSettings {
  const mode = MOVESET_MODES[num(val['pms'])] ?? 'Unchanged';
  const on = (key: string) => mode !== 'Unchanged' && !!val[key];
  return {
    mode,
    guaranteedLevelOneMoves: on('pms.l1.on') ? num(val['pms.l1.v']) : null,
    reorderDamaging: on('pms.ord.on'),
    noGameBreaking: on('pms.ngb.on'),
    goodDamagingPercent: on('pms.good.on') ? num(val['pms.good.v']) : null,
    evolutionMovesForAll: on('pms.evo.on')
  };
}

function tms(val: Values): TmSettings {
  const random = num(val['tm']) === 1;
  return {
    random,
    keepFieldMoves: random && !!val['tm.field.on'],
    noGameBreaking: random && !!val['tm.ngb.on'],
    goodDamagingPercent: random && val['tm.good.on'] ? num(val['tm.good.v']) : null
  };
}

/** Compatibility modes in the order of the UI's radio buttons. */
const COMPAT_MODES: readonly CompatMode[] = ['Unchanged', 'RandomPreferSameType', 'RandomCompletely', 'Full'];

function tmHmCompat(val: Values): TmHmCompatSettings {
  const mode = COMPAT_MODES[num(val['thc'])] ?? 'Unchanged';
  const levelUpSanity = !!val['thc.san.on'];
  return {
    mode,
    levelUpSanity,
    followEvolutions: !!val['thc.fe.on'] && (mode !== 'Unchanged' || levelUpSanity),
    fullHm: mode !== 'Full' && !!val['thc.hm.on']
  };
}

function tutorCompat(val: Values): TutorCompatSettings {
  const mode = COMPAT_MODES[num(val['mtc'])] ?? 'Unchanged';
  const levelUpSanity = !!val['mtc.san.on'];
  return {
    mode,
    levelUpSanity,
    followEvolutions: !!val['mtc.fe.on'] && (mode !== 'Unchanged' || levelUpSanity)
  };
}

function tutors(val: Values): TutorSettings {
  const random = num(val['mt']) === 1;
  return {
    random,
    keepFieldMoves: random && !!val['mt.field.on'],
    noGameBreaking: random && !!val['mt.ngb.on'],
    goodDamagingPercent: random && val['mt.good.on'] ? num(val['mt.good.v']) : null
  };
}

/** Field item modes in the order of the "Field Items" group. */
const FIELD_ITEM_MODES: readonly FieldItemMode[] = ['Unchanged', 'Shuffle', 'Random', 'RandomEven'];

function fieldItems(val: Values): FieldItemSettings {
  const mode = FIELD_ITEM_MODES[num(val['fi'])] ?? 'Unchanged';
  return { mode, banBad: (mode === 'Random' || mode === 'RandomEven') && !!val['fi.ban.on'] };
}

/** Shop item modes in the order of the "Shop Items" group. */
const SHOP_ITEM_MODES: readonly ShopItemMode[] = ['Unchanged', 'Shuffle', 'Random'];

function shops(val: Values): ShopItemSettings {
  const mode = SHOP_ITEM_MODES[num(val['sh'])] ?? 'Unchanged';
  const random = (key: string) => mode === 'Random' && !!val[key];
  return {
    mode,
    banOverpowered: random('sh.op.on'),
    banBad: random('sh.bad.on'),
    banRegular: random('sh.reg.on'),
    guaranteeEvolutionItems: random('sh.evo.on'),
    guaranteeXItems: random('sh.x.on'),
    balancePrices: !!val['sh.price.on'],
    addCheapRareCandies: !!val['sh.candy.on']
  };
}

function pickup(val: Values): PickupItemSettings {
  const random = num(val['pu']) === 1;
  return { random, banBad: random && !!val['pu.ban.on'] };
}

function palettes(val: Values): PaletteSettings {
  const random = num(val['ppal']) === 1;
  return {
    random,
    followTypes: random && !!val['ppal.types.on'],
    followEvolutions: random && !!val['ppal.evo.on'],
    shinyFromNormal: random && !!val['ppal.shiny.on']
  };
}

/** Custom player graphics: the chosen pack when the group is on Custom. */
function playerGraphics(val: Values): PlayerGraphicsSettings {
  const pack = num(val['cpg']) === 1 && typeof val['cpg.pack'] === 'string' ? val['cpg.pack'] : null;
  return { pack, replace: pack && num(val['cpg.who.v']) === 1 ? 'pc2' : 'pc1' };
}

/** Type effectiveness modes in the order of the "Type Effectiveness" group. */
const TYPE_EFFECTIVENESS_MODES: readonly TypeEffectivenessMode[] = ['Unchanged', 'Random', 'RandomBalanced', 'KeepIdentities', 'Inverse'];

function typeEffectiveness(val: Values): TypeEffectivenessSettings {
  const mode = TYPE_EFFECTIVENESS_MODES[num(val['te'])] ?? 'Unchanged';
  return { mode, randomImmunities: mode === 'Inverse' && !!val['te.imm.on'], update: !!val['te.upd.on'] };
}

/** Trainer modes in the order of the "Trainer Pokémon" group. */
const TRAINER_MODES: readonly TrainerMode[] = ['Unchanged', 'Random', 'RandomEvenDistribution', 'RandomEvenDistributionMainGame', 'TypeThemed', 'TypeThemedEliteFourGyms', 'KeepThemed', 'KeepThemesOrPrimary'];

function trainers(val: Values): TrainerSettings {
  const on = (key: string) => !!val['tp.' + key + '.on'];
  const tiers = (row: number) => [0, 1, 2].map((col) => 'tp.t' + row + col);
  return {
    mode: TRAINER_MODES[num(val['tp'])] ?? 'Unchanged',
    levelModifier: optional(val, 'tp.lvl'),
    evolve: optional(val, 'tp.evo'),
    rivalCarriesStarter: on('rival'),
    similarStrength: on('sim'),
    avoidDuplicates: on('dup'),
    weightTypes: on('weight'),
    noLegendaries: on('noleg'),
    noEarlyWonderGuard: on('wg'),
    localPokemon: on('local'),
    randomShiny: on('shiny'),
    eliteFourUnique: optional(val, 'tp.e4'),
    additionalPokemon: tiers(0).map((k) => (val[k + '.on'] ? num(val[k + '.v']) : 0)) as [number, number, number],
    diverseTypes: tiers(2).map((k) => !!val[k + '.on']) as [boolean, boolean, boolean],
    betterMovesets: tiers(3).map((k) => !!val[k + '.on']) as [boolean, boolean, boolean],
    heldItems: tiers(1).map((k) => !!val[k + '.on']) as [boolean, boolean, boolean],
    consumableItemsOnly: on('cons'),
    sensibleItemsOnly: on('sens'),
    highestLevelGetsItem: on('high'),
    randomNames: on('names'),
    randomClassNames: on('classes'),
    allowAltFormes: on('alt'),
    swapMegaEvos: on('mega')
  };
}

/** Battle style modes in the order of the "Battle Style" group, and styles in the order of its "Style" choices. */
const BATTLE_STYLE_MODES: readonly BattleStyleMode[] = ['Unchanged', 'Random', 'SingleStyle'];
const BATTLE_STYLES: readonly BattleStyle[] = ['Single', 'Double', 'Triple', 'Rotation'];

function battleStyle(val: Values): BattleStyleSettings {
  const mode = BATTLE_STYLE_MODES[num(val['bs'])] ?? 'Unchanged';
  return { mode, style: (mode === 'SingleStyle' && BATTLE_STYLES[num(val['bs.style.v'])]) || 'Single' };
}

/** Trade modes in the order of the "In-Game Trades" group. */
const TRADE_MODES: readonly TradeMode[] = ['Unchanged', 'Given', 'GivenAndRequested'];

function trades(val: Values): TradeSettings {
  const mode = TRADE_MODES[num(val['igt'])] ?? 'Unchanged';
  const on = (key: string) => mode !== 'Unchanged' && !!val[key];
  return { mode, randomNicknames: on('igt.nick.on'), randomOts: on('igt.ot.on'), randomIvs: on('igt.iv.on'), randomItems: on('igt.item.on') };
}

/** The generations Limit Pokémon allows, from its dialog. */
export interface Limit {
  /** Truthy for each generation to include, generation 1 first. */
  inc: number[];
  /** Whether evolutionary relatives from other generations are allowed too. */
  related: boolean;
}

/**
 * General options. The intro Pokémon is randomized unless "No Random Intro Mon" is ticked. Limit Pokémon only
 * counts the generations up to the game's own.
 */
function general(val: Values, genNum: number, limit?: Limit): GeneralSettings {
  const out: GeneralSettings = {
    randomIntroPokemon: !val['gen.intro.on'],
    noPrematureEvolutions: !!val['gen.prem.on'],
    noIrregularAltFormes: !!val['gen.irr.on']
  };
  if (val['gen.limit.on'] && limit) {
    const generations = Array.from({ length: 7 }, (_, i) => i < genNum && !!limit.inc[i]);
    out.limitPokemon = { generations: generations as GenerationLimit['generations'], allowEvolutionaryRelatives: limit.related };
  }
  return out;
}

/**
 * The settings the Rust backend runs with, from the effective option values of a game of generation `genNum`. Only
 * ported options are read.
 */
export function toSettings(val: Values, genNum: number, genLimit?: Limit): Settings {
  const mode = STARTER_MODES[num(val['sp'])] ?? 'Unchanged';
  return {
    abilities: abilities(val),
    evolutions: evolutions(val),
    baseStats: baseStats(val, genNum),
    bsts: bsts(val),
    general: general(val, genNum, genLimit),
    customNames: null,
    miscTweaks: TWEAKS.filter(([id]) => !!val['tw.' + id]).map(([id]) => id as MiscTweak),
    expCurves: expCurves(val),
    moveData: moveData(val, genNum),
    movesets: movesets(val),
    tms: tms(val),
    tmHmCompat: tmHmCompat(val),
    tutors: tutors(val),
    tutorCompat: tutorCompat(val),
    starters: {
      mode,
      custom: mode === 'Custom' ? ['sp.s1.v', 'sp.s2.v', 'sp.s3.v'].map((k) => dexNumber(val[k])) : [],
      types: starterTypes(val),
      noLegendaries: !!val['sp.noleg.on'],
      noDualTypes: !!val['sp.nodual.on'],
      bstMin: limit(val, 'sp.bmin'),
      bstMax: limit(val, 'sp.bmax'),
      randomHeldItems: !!val['sp.held.on'],
      banBadItems: !!val['sp.held.on'] && !!val['sp.ban.on'],
      allowAltFormes: !!val['sp.alt.on']
    },
    statics: statics(val),
    totems: totems(val),
    trades: trades(val),
    fieldItems: fieldItems(val),
    shops: shops(val),
    pickup: pickup(val),
    palettes: palettes(val),
    playerGraphics: playerGraphics(val),
    typeEffectiveness: typeEffectiveness(val),
    battleStyle: battleStyle(val),
    trainers: trainers(val),
    types: types(val),
    wild: wild(val)
  };
}

/** What `fromSettings` made of settings from a string or file. */
export interface LoadedOverrides {
  /** The option choices that give back these settings. */
  ov: Overrides;
  /** Limit Pokémon's dialog, when the settings limit the Pokémon. */
  limit: Limit | null;
  /** A custom starter isn't a Pokémon of this game and was left at the game's own. */
  starterUnavailable: boolean;
}

const index = <T>(list: readonly T[], value: T) => Math.max(0, list.indexOf(value));
const flag = (ov: Overrides, key: string, on: boolean) => (ov[key + '.on'] = on);
/** An option with its own checkbox: on with `value`, or off. */
function toggled(ov: Overrides, key: string, value: number | null | undefined, off: number | null = null) {
  const on = value !== null && value !== undefined && value !== off;
  ov[key + '.on'] = on;
  if (on) ov[key + '.v'] = value;
}

/**
 * The inverse of `toSettings`: the option choices that give back `s` in a game of `rom`, for settings loaded from a
 * string or file. Custom player graphics are not part of them.
 */
export function fromSettings(s: Settings, rom: { genNum: number; dexSize: number }, raceMode = false): LoadedOverrides {
  const ov: Overrides = {};
  const { genNum } = rom;

  const g = s.general;
  flag(ov, 'gen.intro', !g.randomIntroPokemon);
  flag(ov, 'gen.prem', g.noPrematureEvolutions);
  flag(ov, 'gen.irr', g.noIrregularAltFormes);
  flag(ov, 'gen.limit', !!g.limitPokemon);
  flag(ov, 'gen.race', raceMode);
  const limit = g.limitPokemon ? { inc: g.limitPokemon.generations.map((on) => (on ? 1 : 0)), related: g.limitPokemon.allowEvolutionaryRelatives } : null;

  for (const [id] of TWEAKS) ov['tw.' + id] = s.miscTweaks.includes(id as MiscTweak);

  const bs = s.baseStats;
  ov['pbs'] = index(BASE_STAT_MODES, bs.mode);
  flag(ov, 'pbs.fe', bs.followEvolutions);
  flag(ov, 'pbs.fm', bs.followMegas);
  flag(ov, 'pbs.ae', bs.randomEvolutionStats);
  toggled(ov, 'pbs.upd', bs.updateToGeneration === null ? null : bs.updateToGeneration - Math.max(6, genNum + 1));

  const bst = s.bsts;
  ov['pbst'] = index(BST_MODES, bst.mode);
  if (bst.mode === 'BuffNerf') ov['pbst.bn.v'] = bst.maxBuffNerfPercent;
  flag(ov, 'pbst.fe', bst.followEvolutions);
  flag(ov, 'pbst.sl', bst.separateLegendaries);

  ov['pt'] = index(TYPE_MODES, s.types.mode);
  flag(ov, 'pt.dual', s.types.forceDual);
  flag(ov, 'pt.fm', s.types.followMegas);

  const a = s.abilities;
  ov['pa'] = a.mode === 'Random' ? 1 : 0;
  flag(ov, 'pa.wg', a.allowWonderGuard);
  flag(ov, 'pa.fe', a.followEvolutions);
  flag(ov, 'pa.fm', a.followMegas);
  flag(ov, 'pa.dup', a.combineDuplicates);
  flag(ov, 'pa.two', a.ensureTwo);
  flag(ov, 'pa.trap', a.banTrapping);
  flag(ov, 'pa.neg', a.banNegative);
  flag(ov, 'pa.bad', a.banBad);

  toggled(ov, 'exp.std', s.expCurves.standardize === null ? null : index(EXP_CURVES, s.expCurves.standardize));
  ov['exp.who.v'] = index(EXP_SCOPES, s.expCurves.applyTo);

  const e = s.evolutions;
  ov['pe'] = index(EVOLUTION_MODES, e.mode);
  flag(ov, 'pe.ss', e.similarStrength);
  flag(ov, 'pe.st', e.sameTyping);
  flag(ov, 'pe.l3', e.limitThreeStages);
  flag(ov, 'pe.fc', e.forceChange);
  flag(ov, 'pe.fg', e.forceGrowth);
  flag(ov, 'pe.nc', e.noConvergence);
  flag(ov, 'pe.alt', e.allowAltFormes);
  flag(ov, 'pe.adj', e.adjustLevels);
  flag(ov, 'pe.ci', e.changeImpossible);
  toggled(ov, 'pe.easy', e.easierMaxLevel);
  flag(ov, 'pe.est', e.useEstimatedLevels);
  flag(ov, 'pe.time', e.removeTimeBased);

  const md = s.moveData;
  flag(ov, 'md.pow', md.power);
  flag(ov, 'md.acc', md.accuracy);
  flag(ov, 'md.pp', md.pp);
  flag(ov, 'md.type', md.types);
  flag(ov, 'md.cat', md.category);
  flag(ov, 'md.name', md.names);
  toggled(ov, 'md.upd', md.updateToGeneration === null ? null : md.updateToGeneration - genNum - 1);

  const ms = s.movesets;
  ov['pms'] = index(MOVESET_MODES, ms.mode);
  toggled(ov, 'pms.l1', ms.guaranteedLevelOneMoves);
  flag(ov, 'pms.ord', ms.reorderDamaging);
  flag(ov, 'pms.ngb', ms.noGameBreaking);
  toggled(ov, 'pms.good', ms.goodDamagingPercent);
  flag(ov, 'pms.evo', ms.evolutionMovesForAll);

  for (const [key, t] of [['tm', s.tms], ['mt', s.tutors]] as const) {
    ov[key] = t.random ? 1 : 0;
    flag(ov, key + '.field', t.keepFieldMoves);
    flag(ov, key + '.ngb', t.noGameBreaking);
    toggled(ov, key + '.good', t.goodDamagingPercent);
  }
  ov['thc'] = index(COMPAT_MODES, s.tmHmCompat.mode);
  flag(ov, 'thc.san', s.tmHmCompat.levelUpSanity);
  flag(ov, 'thc.fe', s.tmHmCompat.followEvolutions);
  flag(ov, 'thc.hm', s.tmHmCompat.fullHm);
  ov['mtc'] = index(COMPAT_MODES, s.tutorCompat.mode);
  flag(ov, 'mtc.san', s.tutorCompat.levelUpSanity);
  flag(ov, 'mtc.fe', s.tutorCompat.followEvolutions);

  ov['fi'] = index(FIELD_ITEM_MODES, s.fieldItems.mode);
  flag(ov, 'fi.ban', s.fieldItems.banBad);
  const sh = s.shops;
  ov['sh'] = index(SHOP_ITEM_MODES, sh.mode);
  flag(ov, 'sh.op', sh.banOverpowered);
  flag(ov, 'sh.bad', sh.banBad);
  flag(ov, 'sh.reg', sh.banRegular);
  flag(ov, 'sh.evo', sh.guaranteeEvolutionItems);
  flag(ov, 'sh.x', sh.guaranteeXItems);
  flag(ov, 'sh.price', sh.balancePrices);
  flag(ov, 'sh.candy', sh.addCheapRareCandies);
  ov['pu'] = s.pickup.random ? 1 : 0;
  flag(ov, 'pu.ban', s.pickup.banBad);

  const p = s.palettes;
  ov['ppal'] = p.random ? 1 : 0;
  flag(ov, 'ppal.types', p.followTypes);
  flag(ov, 'ppal.evo', p.followEvolutions);
  flag(ov, 'ppal.shiny', p.shinyFromNormal);

  ov['te'] = index(TYPE_EFFECTIVENESS_MODES, s.typeEffectiveness.mode);
  flag(ov, 'te.imm', s.typeEffectiveness.randomImmunities);
  flag(ov, 'te.upd', s.typeEffectiveness.update);

  const t = s.trainers;
  ov['tp'] = index(TRAINER_MODES, t.mode);
  toggled(ov, 'tp.lvl', t.levelModifier);
  toggled(ov, 'tp.evo', t.evolve);
  toggled(ov, 'tp.e4', t.eliteFourUnique);
  const trainerFlags = { rival: t.rivalCarriesStarter, sim: t.similarStrength, dup: t.avoidDuplicates, weight: t.weightTypes, noleg: t.noLegendaries, wg: t.noEarlyWonderGuard, local: t.localPokemon, shiny: t.randomShiny, cons: t.consumableItemsOnly, sens: t.sensibleItemsOnly, high: t.highestLevelGetsItem, names: t.randomNames, classes: t.randomClassNames, alt: t.allowAltFormes, mega: t.swapMegaEvos };
  for (const [key, on] of Object.entries(trainerFlags)) flag(ov, 'tp.' + key, on);
  for (let col = 0; col < 3; col++) {
    toggled(ov, 'tp.t0' + col, t.additionalPokemon[col], 0);
    flag(ov, 'tp.t1' + col, t.heldItems[col]);
    flag(ov, 'tp.t2' + col, t.diverseTypes[col]);
    flag(ov, 'tp.t3' + col, t.betterMovesets[col]);
  }
  ov['bs'] = index(BATTLE_STYLE_MODES, s.battleStyle.mode);
  if (s.battleStyle.mode === 'SingleStyle') ov['bs.style.v'] = index(BATTLE_STYLES, s.battleStyle.style);

  const st = s.starters;
  ov['sp'] = index(STARTER_MODES, st.mode);
  let starterUnavailable = false;
  if (st.mode === 'Custom') {
    st.custom.slice(0, 3).forEach((n, i) => {
      if (n !== null && (n > rom.dexSize || !DEX[n - 1])) starterUnavailable = true;
      else ov['sp.s' + (i + 1) + '.v'] = n === null ? 'Random' : DEX[n - 1];
    });
  }
  const types = st.types;
  if (typeof types === 'object') {
    ov['sp.type.v'] = 3;
    ov['sp.single.v'] = types.SingleType === null ? 0 : TYPES.indexOf(types.SingleType) + 1;
  } else {
    ov['sp.type.v'] = { None: 0, FireWaterGrass: 1, TypeTriangle: 2, Unique: 4 }[types];
  }
  flag(ov, 'sp.noleg', st.noLegendaries);
  flag(ov, 'sp.nodual', st.noDualTypes);
  toggled(ov, 'sp.bmin', st.bstMin, 0);
  toggled(ov, 'sp.bmax', st.bstMax, 0);
  flag(ov, 'sp.held', st.randomHeldItems);
  flag(ov, 'sp.ban', st.banBadItems);
  flag(ov, 'sp.alt', st.allowAltFormes);

  const sp = s.statics;
  ov['stp'] = index(STATIC_MODES, sp.mode);
  flag(ov, 'stp.lim', sp.limitMainGameLegendaries);
  flag(ov, 'stp.bst', sp.randomize600Bst);
  flag(ov, 'stp.music', sp.fixMusic);
  flag(ov, 'stp.alt', sp.allowAltFormes);
  flag(ov, 'stp.mega', sp.swapMegaEvos);
  toggled(ov, 'stp.lvl', sp.levelModifier, 0);

  const tt = s.totems;
  ov['totp'] = index(TOTEM_MODES, tt.mode);
  ov['totp.ally.v'] = index(TOTEM_MODES, tt.allyMode);
  ov['totp.aura.v'] = index(AURA_MODES, tt.auraMode);
  flag(ov, 'totp.held', tt.randomizeHeldItems);
  flag(ov, 'totp.alt', tt.allowAltFormes);
  toggled(ov, 'totp.lvl', tt.levelModifier, 0);

  const tr = s.trades;
  ov['igt'] = index(TRADE_MODES, tr.mode);
  flag(ov, 'igt.nick', tr.randomNicknames);
  flag(ov, 'igt.ot', tr.randomOts);
  flag(ov, 'igt.iv', tr.randomIvs);
  flag(ov, 'igt.item', tr.randomItems);

  const w = s.wild;
  ov['wp'] = w.mode === 'Random' ? 1 : 0;
  ov['wp.zone.v'] = index(WILD_ZONES, w.zones);
  flag(ov, 'wp.split', w.splitByKind);
  ov['wp.tr.v'] = index(WILD_TYPES, w.types);
  flag(ov, 'wp.themes', w.keepThemes);
  ov['wp.er.v'] = index(WILD_EVOLUTION, w.evolution);
  flag(ov, 'wp.rel', w.keepRelations);
  flag(ov, 'wp.sim', w.similarStrength);
  flag(ov, 'wp.bal', w.balanceLowLevels);
  flag(ov, 'wp.cea', w.catchEmAll);
  flag(ov, 'wp.noleg', w.noLegendaries);
  toggled(ov, 'wp.catch', w.minCatchRate, 0);
  toggled(ov, 'wp.lvl', w.levelModifier, 0);
  flag(ov, 'wp.held', w.randomHeldItems);
  flag(ov, 'wp.ban', w.banBadHeldItems);
  flag(ov, 'wp.time', w.timeOfDay);
  flag(ov, 'wp.alt', w.allowAltFormes);

  return { ov, limit, starterUnavailable };
}
