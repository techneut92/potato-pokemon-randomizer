//! Copy-up planning: the order in which randomizers treat species so that evolutions can copy what their pre-evolutions
//! got.
//!
//! Instead of calling actions, [`plan`] returns them in order; the caller carries them out.
//!
//! Deliberate choices:
//! - Species are walked in national order, so plans do not depend on how a set was built.
//! - An evolution copies from the first pre-evolution in the set, never from one outside it; the original takes the
//!   first pre-evolution of all, even one outside the set.
//!
//! Alternate formes are planned as the original plans them: a forme whose base species is in the set copies from it
//! ([`CopyUp::AltForme`], or [`CopyUp::Cosmetic`] for a forme that only differs in looks), unless it is an evolution
//! itself (Wormadam's formes, by Burmy's links), and only a base forme, or a forme whose base species is outside the
//! set, can be basic. Without evolution sanity an alternate forme always copies from its base species; the original
//! has it copy from a pre-evolution even then (Wormadam's formes from Burmy).
//!
//! [`plan_with_megas`] can count Mega Evolutions as evolutions of their base species, as the original does for the
//! "Follow Mega Evolutions" options: with evolution sanity a Mega Evolution is then [`CopyUp::Evolved`] (and final),
//! and one whose base species has more than one Mega Evolution (Charizard's, Mewtwo's) is a split evolution.

use std::collections::BTreeSet;

use ppr_model::{DexNumber, GameData, Species};

use super::common::SpeciesSet;

/// What to do with one species, in plan order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CopyUp {
    /// A species with nothing to copy from: it is randomized on its own.
    Basic(DexNumber),
    /// An evolution, after `from` has been treated: it copies from `from`, as a split evolution when `split`.
    Evolved {
        from: DexNumber,
        to: DexNumber,
        /// `to` evolves no further.
        final_evo: bool,
        /// `to` shares its pre-evolution with another evolution, and split evolutions are copied separately.
        split: bool,
    },
    /// An alternate forme, after its base species has been treated: it copies from it.
    AltForme { base: DexNumber, forme: DexNumber },
    /// An alternate forme that only differs in looks, after its base species has been treated: it copies everything.
    Cosmetic { base: DexNumber, forme: DexNumber },
}

/// What a plan follows.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Options {
    /// Evolutions copy from their pre-evolutions; without it every species is basic.
    pub(crate) evolution_sanity: bool,
    /// Split evolutions get [`CopyUp::Evolved`] with `split`; without it they are basic.
    pub(crate) copy_split_evos: bool,
}

/// Pre-evolutions of `n` within `set`, in the game's order.
fn pre_evolutions<'a>(game: &'a GameData, set: &'a SpeciesSet, n: DexNumber) -> impl Iterator<Item = DexNumber> + 'a {
    game.evolutions
        .iter()
        .filter(move |e| e.to == n && set.contains(&e.from))
        .map(|e| e.from)
}

/// What `n` itself evolves into within `set` (not what its base forme evolves into).
fn evolutions_in(game: &GameData, set: &SpeciesSet, n: DexNumber) -> BTreeSet<DexNumber> {
    game.evolutions
        .iter()
        .filter(|e| e.from == n && set.contains(&e.to))
        .map(|e| e.to)
        .collect()
}

/// With `megas`, the base species in `set` that Mega Evolution `n` counts as an evolution of.
fn mega_base(game: &GameData, set: &SpeciesSet, n: DexNumber, megas: bool) -> Option<DexNumber> {
    if !megas || !game.species(n).is_some_and(Species::is_mega) {
        return None;
    }
    base_in(game, set, n)
}

fn is_basic(game: &GameData, set: &SpeciesSet, n: DexNumber, megas: bool) -> bool {
    pre_evolutions(game, set, n).next().is_none() && mega_base(game, set, n, megas).is_none()
}

/// A split evolution: a pre-evolution in the set evolves into more than one species in the set; with `megas`, also
/// a Mega Evolution whose base species has more than one, counting those outside the set as the original does.
fn is_split(game: &GameData, set: &SpeciesSet, n: DexNumber, megas: bool) -> bool {
    pre_evolutions(game, set, n).any(|from| evolutions_in(game, set, from).len() > 1)
        || mega_base(game, set, n, megas).is_some_and(|base| {
            game.species
                .iter()
                .filter(|sp| sp.is_mega() && sp.conceptual_base() == Some(base))
                .count()
                > 1
        })
}

/// A final evolution: not basic, and evolving into nothing in the set.
fn is_final(game: &GameData, set: &SpeciesSet, n: DexNumber, megas: bool) -> bool {
    !is_basic(game, set, n, megas) && evolutions_in(game, set, n).is_empty()
}

/// The base species of alternate forme `n` when it is in `set`.
fn base_in(game: &GameData, set: &SpeciesSet, n: DexNumber) -> Option<DexNumber> {
    game.species(n)
        .and_then(Species::conceptual_base)
        .filter(|b| set.contains(b))
}

fn is_cosmetic(game: &GameData, n: DexNumber) -> bool {
    game.species(n).is_some_and(|s| s.forme.cosmetic)
}

/// What `to` copies from: with evolution sanity its first pre-evolution in the set, unless it only differs in looks
/// from its base species, else its base species.
fn relation_into(game: &GameData, set: &SpeciesSet, to: DexNumber, sanity: bool) -> Option<DexNumber> {
    pre_evolutions(game, set, to)
        .next()
        .filter(|_| sanity && !is_cosmetic(game, to))
        .or_else(|| base_in(game, set, to))
}

/// The plan: every species of `set` once, each evolution and alternate forme after what it copies from. Mega
/// Evolutions are alternate formes.
pub(crate) fn plan(game: &GameData, set: &SpeciesSet, options: Options) -> Vec<CopyUp> {
    plan_with_megas(game, set, options, false)
}

/// [`plan`], with Mega Evolutions counted as evolutions of their base species when `megas`.
pub(crate) fn plan_with_megas(game: &GameData, set: &SpeciesSet, options: Options, megas: bool) -> Vec<CopyUp> {
    let sanity = options.evolution_sanity;
    let split_evos: SpeciesSet = if sanity {
        set.iter().copied().filter(|&n| is_split(game, set, n, megas)).collect()
    } else {
        SpeciesSet::new()
    };
    let basic: SpeciesSet = set
        .iter()
        .copied()
        .filter(|&n| {
            ((!sanity || is_basic(game, set, n, megas)) && base_in(game, set, n).is_none())
                || (!options.copy_split_evos && split_evos.contains(&n))
        })
        .collect();
    let mut steps: Vec<CopyUp> = basic.iter().map(|&n| CopyUp::Basic(n)).collect();
    let mut processed = basic;
    for &n in set {
        if processed.contains(&n) {
            continue;
        }
        // Down to a treated species, then back up.
        let mut chain = Vec::new();
        let mut to = n;
        loop {
            let Some(from) = relation_into(game, set, to, sanity) else {
                // Not reached, as a species that is not basic has a pre-evolution or base species in the set.
                steps.push(CopyUp::Basic(to));
                processed.insert(to);
                break;
            };
            if let Some(k) = chain.iter().position(|&(_, t)| t == from) {
                // A cycle: where it closes counts as basic, so the walk ends.
                chain.truncate(k);
                steps.push(CopyUp::Basic(from));
                processed.insert(from);
                break;
            }
            chain.push((from, to));
            if processed.contains(&from) {
                break;
            }
            to = from;
        }
        while let Some((from, to)) = chain.pop() {
            if !processed.insert(to) {
                continue;
            }
            let evolved = sanity && !is_basic(game, set, to, megas);
            steps.push(if is_cosmetic(game, to) {
                CopyUp::Cosmetic { base: from, forme: to }
            } else if evolved {
                CopyUp::Evolved {
                    from,
                    to,
                    final_evo: is_final(game, set, to, megas),
                    split: options.copy_split_evos && split_evos.contains(&to),
                }
            } else {
                CopyUp::AltForme { base: from, forme: to }
            });
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use ppr_model::{Evolution, EvolutionType};

    use super::*;
    use crate::randomizers::testing::{emerald, species};

    fn game(evolutions: &[(DexNumber, DexNumber)]) -> GameData {
        let mut numbers: SpeciesSet = evolutions.iter().flat_map(|&(a, b)| [a, b]).collect();
        numbers.insert(1);
        let mut g = emerald(
            numbers
                .iter()
                .map(|&n| species(n, ppr_model::Type::Normal, None, 50))
                .collect(),
        );
        g.evolutions = evolutions
            .iter()
            .map(|&(from, to)| Evolution::new(from, to, EvolutionType::Level, 16))
            .collect();
        g
    }

    fn all(g: &GameData) -> SpeciesSet {
        g.species.iter().map(|s| s.number).collect()
    }

    const SANE: Options = Options {
        evolution_sanity: true,
        copy_split_evos: true,
    };

    fn evolved(from: DexNumber, to: DexNumber, final_evo: bool, split: bool) -> CopyUp {
        CopyUp::Evolved {
            from,
            to,
            final_evo,
            split,
        }
    }

    #[test]
    fn evolutions_follow_what_they_copy_from() {
        // Wurmple (265) → Silcoon (266) → Beautifly (267), Wurmple → Cascoon (268) → Dustox (269), listed out of
        // order.
        let g = game(&[(266, 267), (265, 266), (268, 269), (265, 268)]);
        assert_eq!(
            plan(&g, &all(&g), SANE),
            [
                CopyUp::Basic(1),
                CopyUp::Basic(265),
                evolved(265, 266, false, true),
                evolved(266, 267, true, false),
                evolved(265, 268, false, true),
                evolved(268, 269, true, false),
            ]
        );
    }

    #[test]
    fn split_evolutions_are_basic_unless_copied() {
        let g = game(&[(265, 266), (265, 268)]);
        let options = Options {
            copy_split_evos: false,
            ..SANE
        };
        assert_eq!(
            plan(&g, &all(&g), options),
            [
                CopyUp::Basic(1),
                CopyUp::Basic(265),
                CopyUp::Basic(266),
                CopyUp::Basic(268)
            ]
        );
    }

    #[test]
    fn without_sanity_all_are_basic() {
        let g = game(&[(1, 2), (2, 3)]);
        let options = Options {
            evolution_sanity: false,
            ..SANE
        };
        assert_eq!(
            plan(&g, &all(&g), options),
            [CopyUp::Basic(1), CopyUp::Basic(2), CopyUp::Basic(3)]
        );
    }

    #[test]
    fn species_outside_the_set_are_ignored() {
        let g = game(&[(1, 2), (2, 3)]);
        let set = SpeciesSet::from([2, 3]);
        assert_eq!(plan(&g, &set, SANE), [CopyUp::Basic(2), evolved(2, 3, true, false)]);
    }

    #[test]
    fn alternate_formes_copy_from_their_base_species() {
        // Burmy (412) → Wormadam (413), with Wormadam's formes 497 and 498 as Burmy's links, Giratina (487) and its
        // Origin Forme (499), and a cosmetic forme (500) of species 1.
        let mut g = game(&[(412, 413)]);
        g.evolutions
            .extend([497, 498].map(|to| Evolution::new(412, to, EvolutionType::None, 0)));
        g.species.push(species(487, ppr_model::Type::Normal, None, 50));
        for (n, base, number, cosmetic) in [
            (497, 413, 1, false),
            (498, 413, 2, false),
            (499, 487, 1, false),
            (500, 1, 1, true),
        ] {
            let mut s = species(n, ppr_model::Type::Normal, None, 50);
            s.forme = ppr_model::Forme {
                base: Some(base),
                number,
                cosmetic,
                ..ppr_model::Forme::default()
            };
            g.species.push(s);
        }
        g.species.sort_by_key(|s| s.number);
        assert_eq!(
            plan(&g, &all(&g), SANE),
            [
                CopyUp::Basic(1),
                CopyUp::Basic(412),
                CopyUp::Basic(487),
                evolved(412, 413, true, true),
                evolved(412, 497, true, true),
                evolved(412, 498, true, true),
                CopyUp::AltForme { base: 487, forme: 499 },
                CopyUp::Cosmetic { base: 1, forme: 500 },
            ]
        );
        let options = Options {
            evolution_sanity: false,
            ..SANE
        };
        let loose = plan(&g, &all(&g), options);
        assert!(loose.contains(&CopyUp::AltForme { base: 413, forme: 497 }));
        assert!(loose.contains(&CopyUp::Cosmetic { base: 1, forme: 500 }));
        assert_eq!(plan(&g, &SpeciesSet::from([499]), SANE), [CopyUp::Basic(499)]);
    }

    /// Kadabra (64) → Alakazam (65) with Mega Alakazam (802), and Charizard (6) with Mega Charizard X (800) and Y
    /// (801).
    fn mega_game() -> GameData {
        let mut g = game(&[(64, 65)]);
        g.species.push(species(6, ppr_model::Type::Normal, None, 50));
        for (n, base, number) in [(800, 6, 1), (801, 6, 2), (802, 65, 1)] {
            let mut s = species(n, ppr_model::Type::Normal, None, 50);
            s.forme = ppr_model::Forme {
                base: Some(base),
                number,
                mega: true,
                ..ppr_model::Forme::default()
            };
            g.species.push(s);
        }
        g.species.sort_by_key(|s| s.number);
        g
    }

    #[test]
    fn mega_evolutions_are_alternate_formes_unless_counted_as_evolutions() {
        let g = mega_game();
        let set = all(&g);
        let formes = plan(&g, &set, SANE);
        assert_eq!(formes, plan_with_megas(&g, &set, SANE, false));
        for (base, forme) in [(6, 800), (6, 801), (65, 802)] {
            assert!(formes.contains(&CopyUp::AltForme { base, forme }));
        }
        assert_eq!(
            plan_with_megas(&g, &set, SANE, true),
            [
                CopyUp::Basic(1),
                CopyUp::Basic(6),
                CopyUp::Basic(64),
                // Still final: as in the original, a Mega Evolution is no evolution the species evolves into.
                evolved(64, 65, true, false),
                evolved(6, 800, true, true),
                evolved(6, 801, true, true),
                evolved(65, 802, true, false),
            ]
        );
        // Split Mega Evolutions are basic unless split evolutions are copied, as the original's.
        let options = Options {
            copy_split_evos: false,
            ..SANE
        };
        let basic = plan_with_megas(&g, &set, options, true);
        assert!(basic.contains(&CopyUp::Basic(800)) && basic.contains(&CopyUp::Basic(801)));
        assert!(basic.contains(&evolved(65, 802, true, false)));
        // Without evolution sanity they stay alternate formes.
        let loose = Options {
            evolution_sanity: false,
            ..SANE
        };
        assert_eq!(plan_with_megas(&g, &set, loose, true), plan(&g, &set, loose));
        // A Mega Evolution without its base species in the set is basic; a split stays split by the game's count.
        assert_eq!(
            plan_with_megas(&g, &SpeciesSet::from([800]), SANE, true),
            [CopyUp::Basic(800)]
        );
        assert_eq!(
            plan_with_megas(&g, &SpeciesSet::from([6, 800]), SANE, true),
            [CopyUp::Basic(6), evolved(6, 800, true, true)]
        );
    }

    #[test]
    fn cycles_end() {
        let g = game(&[(2, 3), (3, 2)]);
        assert_eq!(
            plan(&g, &all(&g), SANE),
            [CopyUp::Basic(1), CopyUp::Basic(2), evolved(2, 3, false, false)]
        );
    }
}
