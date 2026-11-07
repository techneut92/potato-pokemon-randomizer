import { describe, expect, it } from 'vitest';
import { sampleRoms } from './data/roms';
import { namesFromTexts, parseSeed, settingsRomName } from './shared';

describe('parseSeed', () => {
  it('takes whole numbers up to 2^64 - 1', () => {
    expect(parseSeed(' 42 ')).toBe('42');
    expect(parseSeed('18446744073709551615')).toBe('18446744073709551615');
    expect(parseSeed('18446744073709551616')).toBeNull();
  });

  it('takes negative seeds as their 64 bits', () => {
    expect(parseSeed('-1')).toBe('18446744073709551615');
    expect(parseSeed('-9223372036854775808')).toBe('9223372036854775808');
    expect(parseSeed('-9223372036854775809')).toBeNull();
  });

  it('refuses anything else', () => {
    for (const s of ['', 'abc', '1.5', '1e5', '--1']) expect(parseSeed(s)).toBeNull();
  });
});

describe('settingsRomName', () => {
  it('names the game without the accent', () => {
    expect(settingsRomName({ ...sampleRoms()[1], name: 'Pokémon Emerald (U)' })).toBe('Pokemon Emerald (U)');
  });
});

describe('namesFromTexts', () => {
  it('makes a list per tab, trimmed and without blank lines', () => {
    expect(namesFromTexts([' Robin \n\nSasha', 'Chef', '', 'Twins\n  ', 'Pebble'])).toEqual({
      trainerNames: ['Robin', 'Sasha'],
      trainerClasses: ['Chef'],
      doublesTrainerNames: [],
      doublesTrainerClasses: ['Twins'],
      pokemonNicknames: ['Pebble']
    });
  });
});
