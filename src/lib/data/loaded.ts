import type { Capabilities } from '../generated/Capabilities';
import type { LoadedRom } from '../generated/LoadedRom';
import type { Platform } from '../generated/Platform';
import { DEX } from './dex';
import { sampleRoms, type Ported, type RomInfo } from './roms';

/** What the Rust engine randomizes so far. Keep in step with `ppr-random`'s pipeline and `ppr-settings`. */
export const PORTED: Ported = {
  groups: ['gen', 'upd', 'tw', 'pbs', 'pbst', 'pt', 'pa', 'pe', 'md', 'pms', 'tm', 'thc', 'mt', 'mtc', 'fi', 'sh', 'pu', 'ppal', 'cpg', 'te', 'bs', 'tp', 'exp', 'sp', 'wp', 'stp', 'totp', 'igt'],
  // Time-based encounters: Gen 7 has them (see `portedFor`).
  // Alternate formes of trainers, wild, static, starter and Totem Pokémon and of evolutions show where the game has
  // them; Mega Evolutions are followed and swapped wherever the game has them.
  // Trainer Pokémon: shiny Pokémon are stored by Gen 7 only (see `portedFor`).
  off: [
    'tp.shiny', 'wp.time']
};
const NOTHING: Ported = { groups: [], off: [] };

/** Options of `PORTED.off` that some generations do have: Gen 7 stores shiny trainer Pokémon and has wild
 * encounters by time of day. */
const PORTED_IN: Record<number, readonly string[]> = { 7: ['tp.shiny', 'wp.time'] };

/** What the Rust engine randomizes in a game of generation `gen`. */
export function portedFor(gen: number): Ported {
  const on = PORTED_IN[gen] ?? [];
  return on.length ? { ...PORTED, off: PORTED.off.filter((k) => !on.includes(k)) } : PORTED;
}

const PLATFORMS: Record<Platform, string> = {
  GameBoy: 'Game Boy',
  GameBoyAdvance: 'Game Boy Advance',
  NintendoDs: 'Nintendo DS',
  Nintendo3ds: 'Nintendo 3DS'
};
/** Size of the national Pokédex per generation. */
const DEX_SIZE = [151, 251, 386, 493, 649, 721, 807];

/** The option flags of a game whose handler reports its capabilities. */
function capabilityFlags(c: Capabilities) {
  return {
    intro: c.introPokemon,
    heldStarter: c.starterHeldItems,
    tutors: c.moveTutors,
    mapIdx: c.mapIndices,
    locs: c.encounterLocations,
    shopSizes: c.shopSizes,
    timeEnc: c.timeBasedEncounters,
    mega: c.megaEvolutions,
    split: c.physicalSpecialSplit,
    wildAlt: c.wildAltFormes,
    staticAlt: c.staticAltFormes,
    starterAlt: c.starterAltFormes,
    funcFormes: c.functionalFormes,
    totem: c.totemPokemon,
    english: c.english
  };
}

/** Option visibility flags of games that can't be randomized yet: those of a sample game of the same era. */
function flagsFor(gen: number): RomInfo {
  const [platinum, emerald, ultraMoon] = sampleRoms();
  return gen <= 3 ? emerald : gen <= 5 ? platinum : ultraMoon;
}

const extension = (path: string) => /\.[^./\\]+$/.exec(path)?.[0].toLowerCase() ?? '';

/** The UI's view of a ROM opened by the Rust backend. */
export function romInfo(loaded: LoadedRom, path: string): RomInfo {
  const { identity } = loaded;
  const gen = identity.generation;
  const system = gen === 2 ? 'Game Boy Color' : PLATFORMS[identity.platform];
  const starters = loaded.starters.length ? loaded.starters.map((n) => DEX[n - 1] ?? '#' + n) : ['', '', ''];
  return {
    ...flagsFor(gen),
    path,
    fileName: loaded.fileName,
    ported: loaded.randomizable ? portedFor(gen) : NOTHING,
    name: 'Pokémon ' + identity.name,
    meta: `Gen ${gen} · ${system}`,
    code: identity.code,
    gen: 'G' + gen,
    genNum: gen,
    dexSize: loaded.speciesCount || DEX_SIZE[gen - 1],
    starters,
    is3ds: gen >= 6,
    ext: extension(path),
    // What the handler reports, once it could load the game.
    ...(loaded.randomizable ? { mainLeg: loaded.mainGameLegendaries, musicFix: loaded.staticMusicFix, palette: loaded.palettes, palPartial: false, cpg: loaded.playerGraphicsPacks.length > 0, cpgPacks: loaded.playerGraphicsPacks, cpgPc2: loaded.multiplePlayerCharacters, ...capabilityFlags(loaded.capabilities) } : {}),
    tweaks: loaded.randomizable ? [...loaded.tweaks] : [],
    ...(loaded.updateVersion !== null ? { updateVersion: loaded.updateVersion } : {})
  };
}
