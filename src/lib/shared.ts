import type { RomInfo } from './data/roms';
import type { CustomNames } from './generated/CustomNames';

/** The game as settings strings name it: "Pokemon Emerald (U)". */
export function settingsRomName(rom: RomInfo) {
  return rom.name.replace(/^Pokémon /, 'Pokemon ');
}

/**
 * A seed as the backend takes it: a whole number from 0 to 2^64 - 1, or a negative seed (a signed 64-bit number),
 * which counts as its 64 bits. `null` when it is neither.
 */
export function parseSeed(text: string): string | null {
  const t = text.trim();
  if (!/^-?\d+$/.test(t)) return null;
  const n = BigInt(t);
  if (n < -BigInt('9223372036854775808') || n > BigInt('18446744073709551615')) return null;
  return BigInt.asUintN(64, n).toString();
}

/** The Custom Names Editor's lists, in the order of its tabs. */
export const NAME_LISTS = [
  ['Trainer Names', 'trainerNames'],
  ['Trainer Classes', 'trainerClasses'],
  ['Doubles Trainer Names', 'doublesTrainerNames'],
  ['Doubles Trainer Classes', 'doublesTrainerClasses'],
  ['Pokémon Nicknames', 'pokemonNicknames']
] as const satisfies readonly (readonly [string, keyof CustomNames])[];

/** The editor's texts (one name per line) as lists, without blank lines. */
export function namesFromTexts(texts: string[]): CustomNames {
  const list = (i: number) => (texts[i] ?? '').split('\n').map((n) => n.trim()).filter(Boolean);
  return Object.fromEntries(NAME_LISTS.map(([, key], i) => [key, list(i)])) as CustomNames;
}
