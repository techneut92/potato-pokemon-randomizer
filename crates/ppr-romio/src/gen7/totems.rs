//! Totem Pokémon and their allies: static encounters of `StaticPokemon` file 1 at the indices the ROM entry lists
//! (`TotemPokemonIndices`). Besides species, forme, level, held item and moves, a totem's encounter holds its aura
//! at 0x25, the number of allies at 0x27 and, from 0x28 on, the 1-based index of each ally's encounter, four bytes
//! apart. Totems and allies are not part of the plain static encounters.
//!
//! A totem or ally of an alternate forme with an entry of its own (the Totem formes) has that forme's number as its
//! species; formes the species doesn't have are read as forme 0.
//!
//! Deliberate choices:
//! - Only the fields that changed are written, so an unchanged totem or ally stays byte for byte; a held item of 0
//!   and one of 0xFFFF both read as none.
//! - A totem or ally with a new species or forme gets the default moves of its level, as in the original, and is
//!   stored as its base species with its forme. One that keeps its species and forme keeps its stored forme byte
//!   and moves, also when the randomizer happened to choose the same species, where the original resets its moves.
//! - An aura byte past 18, which no game has, reads as no aura instead of failing.
//! - Allies are read once per totem, in encounter order, even when the totem lists one twice.

use ppr_formats::ctr::Decompression;
use ppr_model::{Aura, ItemId, TotemAlly, TotemPokemon};

use super::constants as c;
use super::files::Files;
use super::personal::FormeIndex;
use super::{put_u16, word};
use crate::RomError;

const AURA: usize = 0x25;
const ALLY_COUNT: usize = 0x27;
const ALLIES: usize = 0x28;
const ALLY_STRIDE: usize = 4;
const MOVES: usize = 12;

/// Where the totems and their allies are.
#[derive(Debug, Clone)]
pub(super) struct Totems {
    /// The encounter index of each totem, with those of its allies.
    spots: Vec<(usize, Vec<usize>)>,
    /// Where the alternate formes are.
    index: FormeIndex,
}

/// The fields totems and allies share: the game-data species and forme, level and held item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fields {
    species: u16,
    forme: u8,
    level: u8,
    held_item: Option<ItemId>,
}

impl Fields {
    fn of(species: u16, forme: u8, level: u8, held_item: Option<ItemId>) -> Self {
        Self {
            species,
            forme,
            level,
            held_item,
        }
    }
}

/// The fields of encounter `i`.
fn read_fields(statics: &[u8], i: usize, index: &FormeIndex) -> Result<Fields, RomError> {
    let at = i * c::STATIC_SIZE;
    let byte = |o: usize| {
        statics
            .get(at + o)
            .copied()
            .ok_or_else(|| RomError::Malformed(format!("static encounter {i} cut off")))
    };
    let held = word(statics, at + 4)?;
    let base = word(statics, at)?;
    let forme = index.holder_forme(base, u16::from(byte(2)?));
    Ok(Fields {
        species: index.species_of(base, forme),
        forme,
        level: byte(3)?,
        // 0xFFFF and 0 are both no item.
        held_item: (held != 0xFFFF && held != 0).then_some(held),
    })
}

/// The totems of the encounters at `indices`, each with its allies in encounter order.
pub(super) fn read(
    statics: &[u8],
    indices: &[usize],
    index: FormeIndex,
) -> Result<(Totems, Vec<TotemPokemon>), RomError> {
    let mut spots = Vec::new();
    let mut totems = Vec::new();
    for &i in indices {
        let at = i * c::STATIC_SIZE;
        if statics.len() < at + c::STATIC_SIZE {
            return Err(RomError::Malformed(format!("no Totem Pokémon encounter {i}")));
        }
        let f = read_fields(statics, i, &index)?;
        let count = usize::from(statics[at + ALLY_COUNT]);
        let mut ally_spots: Vec<usize> = (0..count)
            .filter_map(|j| statics.get(at + ALLIES + ALLY_STRIDE * j))
            .map(|&b| usize::from(b.wrapping_sub(1)))
            .collect();
        ally_spots.sort_unstable();
        ally_spots.dedup();
        let mut allies = Vec::new();
        for &a in &ally_spots {
            let af = read_fields(statics, a, &index)?;
            allies.push(TotemAlly {
                species: af.species,
                forme: af.forme,
                level: af.level,
                held_item: af.held_item,
            });
        }
        totems.push(TotemPokemon {
            species: f.species,
            forme: f.forme,
            level: f.level,
            held_item: f.held_item,
            aura: Aura::from_byte(statics[at + AURA]),
            allies,
        });
        spots.push((i, ally_spots));
    }
    Ok((Totems { spots, index }, totems))
}

impl Totems {
    /// Writes the fields of encounter `i` that changed. A new species or forme is stored as its base species with
    /// its forme, and gets the default moves of its level.
    fn write_fields(&self, statics: &mut [u8], i: usize, loaded: Fields, new: Fields) {
        let at = i * c::STATIC_SIZE;
        if (new.species, new.forme) != (loaded.species, loaded.forme) {
            put_u16(statics, at, self.index.base(new.species));
            statics[at + 2] = match self.index.forme_number(new.species) {
                0 => new.forme,
                f => f,
            };
            for m in 0..4 {
                put_u16(statics, at + MOVES + 2 * m, 0);
            }
        }
        if new.level != loaded.level {
            statics[at + 3] = new.level;
        }
        if new.held_item != loaded.held_item {
            put_u16(statics, at + 4, new.held_item.unwrap_or(0xFFFF));
        }
    }

    /// Writes what changed of the totems and their allies.
    pub(super) fn write(
        &self,
        files: &mut Files,
        path: &str,
        loaded: &[TotemPokemon],
        new: &[TotemPokemon],
    ) -> Result<(), RomError> {
        let garc = files.garc_mut(path, &Decompression::None)?;
        let mut statics = garc.file(c::STATICS_FILE)?;
        self.write_into(&mut statics, loaded, new)?;
        garc.set_file(c::STATICS_FILE, statics)?;
        Ok(())
    }

    /// Writes what changed of the totems and their allies into the static encounter file. An ally shared by two
    /// totems gets what the later one says.
    fn write_into(&self, statics: &mut [u8], loaded: &[TotemPokemon], new: &[TotemPokemon]) -> Result<(), RomError> {
        let len = statics.len();
        let check = |i: usize| {
            if len < (i + 1) * c::STATIC_SIZE {
                return Err(RomError::Malformed(format!("no static encounter {i}")));
            }
            Ok(())
        };
        for (&(i, ref ally_spots), (old, t)) in self.spots.iter().zip(loaded.iter().zip(new)) {
            if old == t {
                continue;
            }
            check(i)?;
            self.write_fields(
                statics,
                i,
                Fields::of(old.species, old.forme, old.level, old.held_item),
                Fields::of(t.species, t.forme, t.level, t.held_item),
            );
            if old.aura != t.aura {
                statics[i * c::STATIC_SIZE + AURA] = t.aura.to_byte();
            }
            for ((&a, oa), na) in ally_spots.iter().zip(&old.allies).zip(&t.allies) {
                if oa == na {
                    continue;
                }
                check(a)?;
                self.write_fields(
                    statics,
                    a,
                    Fields::of(oa.species, oa.forme, oa.level, oa.held_item),
                    Fields::of(na.species, na.forme, na.level, na.held_item),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Species 20 has an alternate forme 2 with entry 900; species 19 has two cosmetic formes.
    fn index() -> FormeIndex {
        FormeIndex::of(&[(900, 20, 2)], &[(19, 2)])
    }

    /// Three encounters: a totem at 0 with allies at 2 and 1 (listed in that order), and its allies.
    fn file() -> Vec<u8> {
        let mut f = vec![0u8; 3 * c::STATIC_SIZE];
        let mut put = |i: usize, species: u16, forme: u8, level: u8, held: u16| {
            let at = i * c::STATIC_SIZE;
            put_u16(&mut f, at, species);
            f[at + 2] = forme;
            f[at + 3] = level;
            put_u16(&mut f, at + 4, held);
            put_u16(&mut f, at + MOVES, 33);
        };
        put(0, 20, 2, 20, 0xFFFF);
        put(1, 19, 1, 12, 0);
        put(2, 21, 0, 13, 155);
        f[AURA] = 5;
        f[ALLY_COUNT] = 2;
        f[ALLIES] = 3;
        f[ALLIES + ALLY_STRIDE] = 2;
        f
    }

    fn ally(species: u16, forme: u8, level: u8, held_item: Option<ItemId>) -> TotemAlly {
        TotemAlly {
            species,
            forme,
            level,
            held_item,
        }
    }

    #[test]
    fn totems_read_with_their_allies_in_encounter_order() {
        let (layout, totems) = read(&file(), &[0], index()).expect("reads");
        assert_eq!(layout.spots, [(0, vec![1, 2])]);
        let t = &totems[0];
        // The totem is its alternate forme's entry; the ally keeps its cosmetic forme.
        assert_eq!(
            (t.species, t.forme, t.level, t.held_item, t.aura.to_byte()),
            (900, 2, 20, None, 5)
        );
        assert_eq!(t.allies, [ally(19, 1, 12, None), ally(21, 0, 13, Some(155))]);
    }

    #[test]
    fn a_forme_the_species_does_not_have_reads_as_forme_0() {
        let mut f = file();
        f[c::STATIC_SIZE + 2] = 7;
        let (_, totems) = read(&f, &[0], index()).expect("reads");
        assert_eq!(totems[0].allies[0], ally(19, 0, 12, None));
    }

    #[test]
    fn only_what_changed_is_written() {
        let f = file();
        let (layout, loaded) = read(&f, &[0], index()).expect("reads");
        let mut statics = f.clone();
        layout.write_into(&mut statics, &loaded, &loaded).expect("writes");
        assert_eq!(statics, f, "unchanged");

        let mut new = loaded.clone();
        new[0].aura = Aura::from_byte(18);
        new[0].level = 30;
        new[0].allies[1].species = 25;
        new[0].allies[0].held_item = Some(158);
        layout.write_into(&mut statics, &loaded, &new).expect("writes");
        let (_, back) = read(&statics, &[0], index()).expect("reads");
        assert_eq!(back, new);
        // The totem keeps its forme and moves; the replaced ally gets forme 0 and the default moves.
        assert_eq!((statics[2], word(&statics, MOVES).expect("moves")), (2, 33));
        let ally_at = 2 * c::STATIC_SIZE;
        assert_eq!(
            (statics[ally_at + 2], word(&statics, ally_at + MOVES).expect("moves")),
            (0, 0)
        );
        // The ally that kept its species keeps its forme.
        assert_eq!(statics[c::STATIC_SIZE + 2], 1);
        assert_eq!(word(&statics, c::STATIC_SIZE + 4).expect("held"), 158);
        assert_eq!(statics[AURA], 18);

        // A held item taken away is 0xFFFF.
        let mut none = new.clone();
        none[0].allies[1].held_item = None;
        layout.write_into(&mut statics, &new, &none).expect("writes");
        assert_eq!(word(&statics, ally_at + 4).expect("held"), 0xFFFF);
    }

    #[test]
    fn a_new_forme_is_stored_as_its_base_species_and_forme_number() {
        let f = file();
        let (layout, loaded) = read(&f, &[0], index()).expect("reads");
        let mut statics = f.clone();
        let mut new = loaded.clone();
        // The ally becomes the alternate forme, the totem the base species in a cosmetic forme.
        new[0].allies[1].species = 900;
        new[0].allies[1].forme = 2;
        new[0].species = 19;
        new[0].forme = 1;
        layout.write_into(&mut statics, &loaded, &new).expect("writes");
        let ally_at = 2 * c::STATIC_SIZE;
        assert_eq!(
            (word(&statics, ally_at).expect("species"), statics[ally_at + 2]),
            (20, 2)
        );
        assert_eq!((word(&statics, 0).expect("species"), statics[2]), (19, 1));
        assert_eq!(
            word(&statics, MOVES).expect("moves"),
            0,
            "a new species gets the default moves"
        );
        let (_, back) = read(&statics, &[0], index()).expect("reads");
        assert_eq!(back, new);

        // Only the forme changing also counts as new.
        let mut statics2 = f.clone();
        let mut cosmetic = loaded.clone();
        cosmetic[0].allies[0].forme = 0;
        layout.write_into(&mut statics2, &loaded, &cosmetic).expect("writes");
        assert_eq!(
            (
                statics2[c::STATIC_SIZE + 2],
                word(&statics2, c::STATIC_SIZE + MOVES).expect("moves")
            ),
            (0, 0)
        );
    }
}
