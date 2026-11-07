//! Evolutions (`PokemonEvolutions`): one file per personal entry, eight entries of 8 bytes each: method (u16),
//! parameter (u16), target species (u16), target forme (i8, -1 keeps the forme) and level (u8).
//!
//! An evolution's target is the alternate forme it names when that forme has an entry of its own (Rattata's Alolan
//! Forme evolves into Raticate's), else the species. A species' entry with target forme -1 keeps its forme, which
//! for a base species is the base forme. Alternate formes whose entries keep the forme (Floette's and Pumpkaboo's)
//! evolve like their base species: they are not read, and their entries are written again with the base species'
//! evolutions. Nincada's evolution into Shedinja is not in the table but in the executable, which creates the extra
//! species when Nincada evolves into the species it checks for; it is read as a second evolution of Nincada while
//! Nincada evolves into that species.
//!
//! Burmy also evolves into Wormadam's Sandy and Trash Cloaks and, in Sun and Moon, Pikachu, Exeggcute and Cubone
//! into the Raichu, Exeggutor and Marowak that only evolve elsewhere, all with no method; such evolutions are not
//! written.

use ppr_model::{DexNumber, Evolution, EvolutionType};

use std::collections::BTreeMap;

use super::constants::{self as c, Family};
use super::files::Files;
use super::personal::{Forme, Personal, species_of};
use super::{find, put_u16, put_u32, word};
use crate::RomError;

const ENTRY: usize = 8;

/// One raw entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Raw {
    method: u16,
    info: u16,
    species: u16,
    forme: u8,
    level: u8,
}

impl Raw {
    fn parse(file: &[u8], slot: usize) -> Result<Self, RomError> {
        let at = slot * ENTRY;
        Ok(Self {
            method: word(file, at)?,
            info: word(file, at + 2)?,
            species: word(file, at + 4)?,
            forme: file.get(at + 6).copied().unwrap_or_default(),
            level: file.get(at + 7).copied().unwrap_or_default(),
        })
    }

    fn is_evolution(self) -> bool {
        (1..=c::EVOLUTION_METHOD_COUNT).contains(&self.method) && self.species >= 1
    }
}

/// The evolution tables as loaded.
#[derive(Debug, Clone)]
pub(super) struct Evolutions {
    /// The raw entries of each file that holds evolutions, by personal index.
    raw: Vec<Vec<Raw>>,
    /// The alternate formes that evolve like their species (their entries keep the forme, -1): personal index and
    /// species.
    copy_base: Vec<(usize, DexNumber)>,
    /// The alternate formes by personal index.
    formes: BTreeMap<usize, Forme>,
    family: Family,
    sunny: bool,
}

/// The evolutions with no method that make species count as evolving into others: Burmy into Wormadam's other
/// Cloaks and, in Sun and Moon, Pikachu, Exeggcute and Cubone into their Kanto evolutions.
fn added_evolutions(from: DexNumber, family: Family, formes: &BTreeMap<usize, Forme>) -> Vec<Evolution> {
    let to: Vec<DexNumber> = match (from, family) {
        (c::BURMY, _) => vec![species_of(formes, c::WORMADAM, 1), species_of(formes, c::WORMADAM, 2)],
        (c::PIKACHU, Family::SunMoon) => vec![c::RAICHU],
        (c::EXEGGCUTE, Family::SunMoon) => vec![c::EXEGGUTOR],
        (c::CUBONE, Family::SunMoon) => vec![c::MAROWAK],
        _ => Vec::new(),
    };
    to.into_iter()
        .map(|to| Evolution::new(from, to, EvolutionType::None, 0))
        .collect()
}

/// The type of an evolution method; `None` for methods that are no evolution here.
fn kind_of(raw: Raw, family: Family, sunny: bool) -> Option<EvolutionType> {
    use EvolutionType as E;
    let game = |this: E, other: E| {
        let wants_sunny = raw.info.to_le_bytes()[0] == family.sunny_evolution_info();
        if wants_sunny == sunny { this } else { other }
    };
    match raw.method {
        c::EVOLUTION_LEVEL_GAME => Some(game(E::LevelGameThis, E::LevelGameOther)),
        c::EVOLUTION_LEVEL_GAME_DAY => Some(game(E::LevelGameThisDay, E::LevelGameOtherDay)),
        c::EVOLUTION_LEVEL_GAME_NIGHT => Some(game(E::LevelGameThisNight, E::LevelGameOtherNight)),
        m => c::EVOLUTION_TYPES
            .get(usize::from(m).wrapping_sub(1))
            .copied()
            .flatten(),
    }
}

/// [`kind_of`] for an evolution of `from`: Espurr's female evolution uses a method (left over from Gen 6) that
/// also sets the forme; it is an evolution at a level for females.
fn kind_of_from(from: DexNumber, raw: Raw, family: Family, sunny: bool) -> Option<EvolutionType> {
    if from == c::ESPURR && raw.method == c::MEOWSTIC_F_EVOLUTION_METHOD {
        return Some(EvolutionType::LevelFemaleOnly);
    }
    kind_of(raw, family, sunny)
}

/// The target, type and parameter of entry `r` of `from`; `None` for an entry that is no evolution here, or that
/// keeps the forme of an alternate forme.
fn decode(
    from: DexNumber,
    r: Raw,
    formes: &BTreeMap<usize, Forme>,
    family: Family,
    sunny: bool,
) -> Option<(DexNumber, EvolutionType, u16)> {
    if !r.is_evolution() {
        return None;
    }
    let mut kind = kind_of_from(from, r, family, sunny)?;
    let mut forme = r.forme;
    if forme == 0xFF {
        if formes.contains_key(&usize::from(from)) {
            return None;
        }
        forme = 0;
    }
    if from == c::ESPURR && r.method == c::MEOWSTIC_F_EVOLUTION_METHOD {
        kind = EvolutionType::LevelFemaleOnly;
        forme = 1;
    }
    let parameter = if kind.uses_level_threshold() {
        u16::from(r.level)
    } else {
        r.info
    };
    Some((species_of(formes, r.species, forme), kind, parameter))
}

impl Evolutions {
    pub(super) fn read(
        files: &mut Files,
        path: &str,
        personal: &Personal,
        sunny: bool,
    ) -> Result<(Self, Vec<Evolution>), RomError> {
        let family = personal.family;
        let total = family.pokemon_count() + family.forme_count();
        let garc = files.garc(path, &ppr_formats::ctr::Decompression::None)?;
        let mut raw = vec![Vec::new()];
        for i in 1..=total {
            let file = garc.file(i)?;
            raw.push(
                (0..c::EVOLUTIONS_PER_SPECIES)
                    .map(|slot| Raw::parse(&file, slot))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        let mut evolutions = Vec::new();
        for (n, species) in raw.iter().enumerate().skip(1) {
            let from = DexNumber::try_from(n).unwrap_or_default();
            for &r in species {
                let Some((to, kind, parameter)) = decode(from, r, &personal.formes, family, sunny) else {
                    continue;
                };
                // Feebas's evolution by beauty has no way to happen; it is left out.
                if kind == EvolutionType::HighBeauty {
                    continue;
                }
                evolutions.push(Evolution::new(from, to, kind, parameter));
            }
            evolutions.extend(added_evolutions(from, family, &personal.formes));
            // The executable creates the extra species when Nincada evolves into the primary one.
            if from == c::NINCADA
                && let Some((primary, extra)) = read_shedinja(&files.code)?
                && evolutions.iter().any(|e| e.from == from && e.to == primary)
            {
                evolutions.push(Evolution::new(from, extra, EvolutionType::LevelIsExtra, 20));
            }
        }
        let mut copy_base = Vec::new();
        for (&i, f) in &personal.formes {
            let keeps = raw.get(i).is_some_and(|entries| {
                entries.iter().any(|r| {
                    r.is_evolution() && r.forme == 0xFF && personal.has_forme(r.species, f.forme) && f.forme > 0
                })
            });
            if keeps {
                copy_base.push((i, f.species));
            }
        }
        Ok((
            Self {
                raw,
                copy_base,
                formes: personal.formes.clone(),
                family,
                sunny,
            },
            evolutions,
        ))
    }

    /// The parameter byte of an evolution that only happens in one of the two games.
    fn game_info(&self, kind: EvolutionType) -> u16 {
        use EvolutionType as E;
        let this = matches!(kind, E::LevelGameThis | E::LevelGameThisDay | E::LevelGameThisNight);
        u16::from(if this ^ self.sunny {
            self.family.moony_evolution_info()
        } else {
            self.family.sunny_evolution_info()
        })
    }

    /// The decoded loaded entries of personal entry `index`: target, type, parameter and the raw entry.
    fn decoded(&self, index: usize) -> Vec<(DexNumber, EvolutionType, u16, Raw)> {
        let from = DexNumber::try_from(index).unwrap_or_default();
        self.raw
            .get(index)
            .into_iter()
            .flatten()
            .filter_map(|&r| {
                let (to, kind, parameter) = decode(from, r, &self.formes, self.family, self.sunny)?;
                Some((to, kind, parameter, r))
            })
            .collect()
    }

    /// Writes the evolutions of every species whose evolutions changed, and of the alternate formes that evolve
    /// like it. Nincada's second evolution goes into the executable.
    pub(super) fn write(
        &mut self,
        files: &mut Files,
        path: &str,
        loaded: &[Evolution],
        new: &[Evolution],
    ) -> Result<(), RomError> {
        let count = self.family.pokemon_count() + self.family.forme_count();
        let mut tables: Vec<(usize, Vec<u8>)> = Vec::new();
        let mut changed_species = Vec::new();
        for n in (1..=count).filter(|n| !self.copy_base.iter().any(|&(i, _)| i == *n)) {
            let from = DexNumber::try_from(n).unwrap_or_default();
            let before: Vec<Evolution> = loaded.iter().filter(|e| e.from == from).copied().collect();
            let mut list: Vec<Evolution> = new.iter().filter(|e| e.from == from).copied().collect();
            if list == before {
                continue;
            }
            list.retain(|e| e.kind != EvolutionType::None);
            changed_species.push(n);
            if from == c::NINCADA && list.len() >= 2 {
                write_shedinja(&mut files.code, list[0].to, list[1].to)?;
                list.remove(1);
            }
            tables.push((n, self.table(n, &list, false)));
        }
        for &(i, base) in &self.copy_base {
            if changed_species.contains(&usize::from(base)) {
                let list: Vec<Evolution> = new
                    .iter()
                    .filter(|e| e.from == base && e.kind != EvolutionType::None)
                    .copied()
                    .collect();
                tables.push((i, self.table(i, &list, true)));
            }
        }
        if tables.is_empty() {
            return Ok(());
        }
        let garc = files.garc_mut(path, &ppr_formats::ctr::Decompression::None)?;
        for (i, table) in tables {
            let mut file = garc.file(i)?;
            if file.len() < table.len() {
                file.resize(table.len(), 0);
            }
            file[..table.len()].copy_from_slice(&table);
            garc.set_file(i, file)?;
        }
        Ok(())
    }

    /// The eight entries of personal entry `index` for `list`. Item evolutions at any time are stored as one by
    /// day and one by night; evolutions not at a level store the level of the species' evolution at a level, as the
    /// game does. An evolution that was loaded as it is keeps its entry; `keep_forme` stores -1 as every target forme.
    fn table(&self, index: usize, list: &[Evolution], keep_forme: bool) -> Vec<u8> {
        let mut list = list.to_vec();
        let items: Vec<usize> = (0..list.len())
            .filter(|&i| list[i].kind == EvolutionType::Item)
            .collect();
        for i in items {
            let mut night = list[i];
            list[i].kind = EvolutionType::ItemDay;
            night.kind = EvolutionType::ItemNight;
            list.push(night);
        }
        let level_of_others = list
            .iter()
            .filter(|e| e.kind.uses_level_threshold())
            .map(|e| e.parameter)
            .next_back();
        let mut loaded = self.decoded(index);
        let mut out = vec![0u8; c::EVOLUTIONS_PER_SPECIES * ENTRY];
        for (slot, e) in list.iter().take(c::EVOLUTIONS_PER_SPECIES).enumerate() {
            let at = slot * ENTRY;
            let level = if e.kind.uses_level_threshold() {
                e.parameter
            } else {
                level_of_others.unwrap_or(0)
            };
            let same = loaded
                .iter()
                .position(|&(to, kind, parameter, _)| to == e.to && kind == e.kind && parameter == e.parameter);
            let raw = if let Some(p) = same {
                let (_, _, _, r) = loaded.remove(p);
                Raw {
                    level: u8::try_from(level).unwrap_or(u8::MAX),
                    ..r
                }
            } else {
                let info = if c::is_game_specific(e.kind) {
                    self.game_info(e.kind)
                } else if e.kind.uses_level_threshold() {
                    0
                } else {
                    e.parameter
                };
                Raw {
                    method: c::evolution_method(e.kind).unwrap_or(0),
                    info,
                    species: super::personal::base_of(&self.formes, e.to),
                    forme: self.formes.get(&usize::from(e.to)).map_or(0, |f| f.forme),
                    level: u8::try_from(level).unwrap_or(u8::MAX),
                }
            };
            put_u16(&mut out, at, raw.method);
            put_u16(&mut out, at + 2, raw.info);
            put_u16(&mut out, at + 4, raw.species);
            out[at + 6] = if keep_forme { 0xFF } else { raw.forme };
            out[at + 7] = raw.level;
        }
        out
    }
}

/// The load of the extra species from the constant after the function, once [`write_shedinja`] patched it.
const EXTRA_SPECIES_LOAD: [u8; 4] = [0x6C, 0x10, 0x9F, 0xE5];

/// Nincada's evolutions in the executable: the species it checks for after evolving (Ninjask) and the extra
/// Pokémon it then creates (Shedinja), when the code is there.
fn read_shedinja(code: &[u8]) -> Result<Option<(DexNumber, DexNumber)>, RomError> {
    let Some(primary_at) = find(code, c::NINJASK_SPECIES_PREFIX)? else {
        return Ok(None);
    };
    let primary = word(code, primary_at)?;
    let extra = match find(code, c::SHEDINJA_PREFIX)? {
        Some(at) if code.get(at + 4..at + 8) == Some(&EXTRA_SPECIES_LOAD[..]) => word(code, at + 120)?,
        _ => c::SHEDINJA,
    };
    Ok(Some((primary, extra)))
}

/// Nincada's evolutions in the executable: the species it checks for after evolving (Ninjask) becomes `primary`,
/// and the extra Pokémon it then creates (Shedinja) becomes `extra`. The extra species is loaded from a constant
/// after the function, for which the code after the last no-op moves up 4 bytes; code patched before only gets
/// the new constant.
fn write_shedinja(code: &mut [u8], primary: DexNumber, extra: DexNumber) -> Result<(), RomError> {
    if let Some(at) = find(code, c::NINJASK_SPECIES_PREFIX)? {
        put_u32(code, at, u32::from(primary));
    }
    if let Some(at) = find(code, c::SHEDINJA_PREFIX)? {
        if code.len() < at + 124 {
            return Err(RomError::Malformed("the Shedinja code is cut off".into()));
        }
        if code[at + 4..at + 8] != EXTRA_SPECIES_LOAD {
            code.copy_within(at + 88..at + 124, at + 84);
            // The branches that moved up point 4 bytes further, one instruction.
            for bl in [84, 96, 108] {
                code[at + bl] = code[at + bl].wrapping_add(1);
            }
            // The forme of the extra species (0), and a load of the species from the constant.
            code[at] = 0;
            code[at + 4..at + 8].copy_from_slice(&EXTRA_SPECIES_LOAD);
        }
        put_u32(code, at + 120, u32::from(extra));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
            .collect()
    }

    /// An executable with Nincada's code: the Ninjask check and the Shedinja function, filled with numbered bytes.
    fn code() -> Vec<u8> {
        let mut code = vec![0xAA; 16];
        code.extend(hex(c::NINJASK_SPECIES_PREFIX));
        code.extend(u32::from(c::NINCADA + 1).to_le_bytes());
        code.extend([0xBB; 16]);
        code.extend(hex(c::SHEDINJA_PREFIX));
        code.extend((0..130u8).map(|i| i.wrapping_mul(3)));
        code
    }

    #[test]
    fn the_unpatched_code_creates_shedinja_after_ninjask() {
        assert_eq!(read_shedinja(&code()).expect("reads"), Some((291, c::SHEDINJA)));
        assert_eq!(read_shedinja(&[0; 64]).expect("reads"), None);
    }

    #[test]
    fn the_shedinja_patch_is_read_back_and_applied_once() {
        let mut once = code();
        write_shedinja(&mut once, 4, 7).expect("writes");
        assert_eq!(read_shedinja(&once).expect("reads"), Some((4, 7)));

        let mut twice = once.clone();
        write_shedinja(&mut twice, 1, 10).expect("writes");
        assert_eq!(read_shedinja(&twice).expect("reads"), Some((1, 10)));
        write_shedinja(&mut twice, 4, 7).expect("writes");
        assert_eq!(twice, once, "patching again only changes the species");
    }
}
