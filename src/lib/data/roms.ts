import type { PlayerGraphicsOption } from '../generated/PlayerGraphicsOption';

/** The parts of the randomizer the new engine does for real games: group keys, plus options (`group.opt`) and
 *  choices (`group.opt#index`, `group#mode`) inside those groups that it doesn't do yet. */
export interface Ported {
  groups: readonly string[];
  off: readonly string[];
}

/** A loaded game and what it supports. */
export interface RomInfo {
  /** Where the ROM is; sample ROMs use `sample:<index>`. */
  path: string;
  fileName: string;
  /** What can really be randomized. Absent for the sample ROMs of the mock backend, which pretend everything. */
  ported?: Ported;
  name: string;
  meta: string;
  code: string;
  gen: string;
  genNum: number;
  dexSize: number;
  starters: string[];
  is3ds: boolean;
  ext: string;
  mega: boolean;
  mapIdx: boolean;
  locs: boolean;
  timeEnc: boolean;
  wildAlt: boolean;
  staticAlt: boolean;
  starterAlt: boolean;
  funcFormes: boolean;
  mainLeg: boolean;
  musicFix: boolean;
  intro: boolean;
  heldStarter: boolean;
  tutors: boolean;
  shopSizes: boolean;
  palette: boolean;
  palPartial: boolean;
  cpg: boolean;
  /** The custom player graphics packs the game can use. */
  cpgPacks: PlayerGraphicsOption[];
  /** Custom player graphics can replace a second player character. */
  cpgPc2: boolean;
  split: boolean;
  totem: boolean;
  english: boolean;
  /** Available misc tweak ids, see TWEAKS. */
  tweaks: string[];
  /** With a game update laid over the game: what its title screen shows, e.g. `Ver. 1.5`. */
  updateVersion?: string;
}

/** Sample ROMs used until the Rust backend can read real ones. */
export function sampleRoms(): RomInfo[] {
  const base = { mega: false, mapIdx: false, locs: true, timeEnc: false, wildAlt: false, staticAlt: false, starterAlt: false, funcFormes: false, mainLeg: false, musicFix: false, intro: true, heldStarter: true, tutors: true, shopSizes: true, palette: false, palPartial: false, cpg: false, cpgPacks: [], cpgPc2: false, split: false, totem: false, english: true };
  return [
    Object.assign({}, base, { path: "sample:0", fileName: "[rom-file].nds", name: "Pokémon Platinum (U)", meta: "Gen 4 · Nintendo DS", code: "CPUE", gen: "G4", genNum: 4, dexSize: 493, starters: ["Turtwig", "Chimchar", "Piplup"], is3ds: false, ext: ".nds",
      mapIdx: true, funcFormes: true, mainLeg: true, musicFix: true, palette: true, palPartial: true, split: true,
      tweaks: ["fastestText", "nationalDex", "fastEggHatching", "lowerCaseNames", "catchingTutorial", "luckyEgg", "runWithoutRunningShoes", "fasterHpAndExpBars", "fastDistortionWorld", "updateRotomFormeTyping", "reusableTMs", "forgettableHMs", "noEVYields"] }),
    Object.assign({}, base, { path: "sample:1", fileName: "[rom-file].gba", name: "Pokémon Emerald (U)", meta: "Gen 3 · Game Boy Advance", code: "BPEE", gen: "G3", genNum: 3, dexSize: 386, starters: ["Treecko", "Torchic", "Mudkip"], is3ds: false, ext: ".gba",
      musicFix: true, palette: true, cpg: true, cpgPc2: true,
      tweaks: ["fastestText", "runningShoes", "pcPotion", "nationalDex", "fastEggHatching", "lowerCaseNames", "catchingTutorial", "luckyEgg", "runWithoutRunningShoes", "reusableTMs", "forgettableHMs", "noEVYields"] }),
    Object.assign({}, base, { path: "sample:2", fileName: "[rom-file].3ds", name: "Pokémon Ultra Moon", meta: "Gen 7 · Nintendo 3DS", code: "A2BA", gen: "G7", genNum: 7, dexSize: 807, starters: ["Rowlet", "Litten", "Popplio"], is3ds: true, ext: ".3ds",
      mega: true, timeEnc: true, wildAlt: true, staticAlt: true, starterAlt: true, funcFormes: true, mainLeg: true, intro: false, shopSizes: false, split: true, totem: true,
      tweaks: ["fastestText", "fastEggHatching", "luckyEgg", "sosBattles", "retainAltFormes", "noEVYields"] })
  ] as RomInfo[];
}

/** All misc tweaks, in the order they apply in: [id, label]. */
export const TWEAKS: readonly (readonly [string, string])[] = [["bwPatch", "B/W Exp Patch"], ["nerfXAcc", "Nerf X Accuracy"], ["critRateFix", "“Fix” Crit Rate"], ["fastestText", "Fastest Text"], ["runningShoes", "Running Shoes Indoors"], ["pcPotion", "Randomize PC Potion"], ["pikachuEvo", "Allow Pikachu Evolution"], ["nationalDex", "Give National Dex at Start"], ["fastEggHatching", "Fast Egg Hatching"], ["forceChallengeMode", "Force Challenge Mode"], ["lowerCaseNames", "Lower Case Pokémon Names"], ["catchingTutorial", "Randomize Catching Tutorial"], ["luckyEgg", "Ban Lucky Egg"], ["freeLuckyEgg", "No Free Lucky Egg"], ["maniacItems", "Ban Big Money Maniac Items"], ["sosBattles", "All Wild Pokémon Can Call Allies"], ["balanceStaticLevels", "Balance Static Pokémon Levels"], ["retainAltFormes", "Don’t Revert Temporary Alt Formes"], ["runWithoutRunningShoes", "Run Without Running Shoes"], ["fasterHpAndExpBars", "Faster HP and EXP Bars"], ["fastDistortionWorld", "Fast Distortion World"], ["updateRotomFormeTyping", "Update Rotom Appliance Typings"], ["disableLowHpMusic", "Disable Low HP Music"], ["reusableTMs", "Reusable TMs"], ["forgettableHMs", "Forgettable HMs"], ["noEVYields", "No EVs From Pokémon"]];

export const SUPPORTED_GAMES: readonly [string, string, string[]][] = [
  ['Generation 1', 'Game Boy', ['Red', 'Blue', 'Green (JP)', 'Yellow']],
  ['Generation 2', 'Game Boy Color', ['Gold', 'Silver', 'Crystal']],
  ['Generation 3', 'Game Boy Advance', ['Ruby', 'Sapphire', 'Emerald', 'FireRed', 'LeafGreen']],
  ['Generation 4', 'Nintendo DS', ['Diamond', 'Pearl', 'Platinum', 'HeartGold', 'SoulSilver']],
  ['Generation 5', 'Nintendo DS', ['Black', 'White', 'Black 2', 'White 2']],
  ['Generation 6', 'Nintendo 3DS', ['X', 'Y', 'Omega Ruby', 'Alpha Sapphire']],
  ['Generation 7', 'Nintendo 3DS', ['Sun', 'Moon', 'Ultra Sun', 'Ultra Moon']]
];
